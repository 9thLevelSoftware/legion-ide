//! Tolerant tool-call extraction and alias normalization.
//!
//! Small local models frequently emit tool calls as *prose* rather than as
//! structured provider fields: wrapped in `<tool_call>` tags, inside a
//! fenced code block, in Qwen-style `<|tool_call_start|>` Liquid syntax, or
//! under near-miss names (`Read` instead of `read_file`). A strict provider
//! parser discards all of it and the run stalls with no tool call at all.
//!
//! This module recovers those calls **before** the typed boundary, without
//! inventing data:
//!
//! * Extraction is priority-ordered — tagged, then Liquid, then fenced, then
//!   bare — and only the winning source is consumed.
//! * Repair is deliberately narrow. Trailing commas are stripped because
//!   they are unambiguous; single-quoted keys, Python literals (`True`,
//!   `None`) and truncated JSON are **not** repaired, because guessing at
//!   their intent risks fabricating a tool call the model never made.
//! * Calls naming a tool the model was not offered are dropped rather than
//!   forwarded.
//!
//! Recovery ends here: the result is still only a *request*. Authorization
//! remains with the capability broker, and mutations remain proposal-mediated
//! (ADR-0049).
//!
//! Behavior and the accompanying fixture corpus are derived from SmallCode
//! (<https://github.com/Doorman11991/smallcode>, MIT) — see
//! `THIRD_PARTY_NOTICES.md` and `docs/legal/smallcode-attribution.md`.

use serde_json::{Map, Value};

/// One tool call recovered from model output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedToolCall {
    /// Tool name. Left exactly as the model wrote it unless it had to be
    /// canonicalized to match an offered tool — see [`normalize_alias`].
    pub name: String,
    /// Parsed arguments, or `Value::Null` when the model supplied none.
    pub arguments: Value,
    /// Raw argument text when the model *did* supply arguments but they could
    /// not be parsed.
    ///
    /// Distinguishing this from "no arguments" matters: a call with absent
    /// arguments may be perfectly valid, whereas one with unparseable
    /// arguments must never be dispatched. Callers turn this into a
    /// non-dispatchable malformed block.
    pub arguments_unparsed: Option<String>,
}

/// Result of scanning one assistant message for embedded tool calls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolCallExtraction {
    /// Recovered calls, in the order they appeared.
    pub calls: Vec<ExtractedToolCall>,
    /// Message content with consumed spans removed and trimmed. Callers keep
    /// this as the assistant's prose so recovery does not duplicate text that
    /// has become a structured call.
    pub residual_content: String,
}

/// Inputs for one extraction pass.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractionInput<'a> {
    /// Assistant message content.
    pub content: &'a str,
    /// Reasoning/thinking channel, used only when `content` is blank.
    pub reasoning_content: Option<&'a str>,
    /// True when the provider already returned structured tool calls. Recovery
    /// is skipped entirely so a call is never counted twice.
    pub has_existing_tool_calls: bool,
    /// Tool names the model was offered. Empty disables filtering.
    pub known_tools: &'a [String],
}

/// Recover tool calls embedded in an assistant message.
pub fn extract_tool_calls(input: &ExtractionInput<'_>) -> ToolCallExtraction {
    if input.has_existing_tool_calls {
        return ToolCallExtraction {
            calls: Vec::new(),
            residual_content: input.content.trim().to_string(),
        };
    }

    let source = if input.content.trim().is_empty() {
        input.reasoning_content.unwrap_or("")
    } else {
        input.content
    };
    if source.trim().is_empty() {
        return ToolCallExtraction::default();
    }

    // Priority order: the first source that yields any span wins outright, so
    // a tagged call is never double-counted with a fenced restatement of it.
    let scan = scan_tagged(source)
        .or_else(|| scan_liquid(source))
        .or_else(|| scan_fenced(source))
        .or_else(|| scan_bare(source))
        // Last: edits written as literal SEARCH/REPLACE blocks or diff hunks
        // rather than as a tool call at all. Models trained on those formats
        // emit them unprompted, and without this they read as prose and the
        // edit is lost.
        .or_else(|| scan_edit_blocks(source, input.known_tools));

    let Some(scan) = scan else {
        return ToolCallExtraction {
            calls: Vec::new(),
            residual_content: source.trim().to_string(),
        };
    };

    let calls = scan
        .calls
        .into_iter()
        .filter_map(|call| resolve_against_known(call, input.known_tools))
        .collect();

    ToolCallExtraction {
        calls,
        residual_content: strip_spans(source, &scan.spans),
    }
}

/// Calls plus the source byte ranges they were recovered from.
struct ScanResult {
    calls: Vec<ExtractedToolCall>,
    spans: Vec<(usize, usize)>,
}

impl ScanResult {
    fn non_empty(self) -> Option<Self> {
        if self.spans.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

/// Match a recovered call against the tools the model was actually offered.
///
/// Candidates are tried in order and the first one the registry offers wins:
///
/// 1. **The name as written.** A literal match always wins, so a registry
///    exposing `shell` keeps receiving `shell` rather than being rewritten to
///    `bash`.
/// 2. **The SmallCode canonical name** ([`normalize_alias`]) — for registries
///    that use that vocabulary.
/// 3. **Legion's own registry name** ([`legion_registry_name`]) — the case
///    that actually matters in a delegated run, where the offered tools are
///    `read`, `grep`, `glob`, `outline`, `edit-as-proposal`,
///    `terminal-command`.
///
/// Without step 3 the alias layer is inert in production: a model writing
/// `Read` canonicalizes to SmallCode's `read_file`, which Legion does not
/// offer, and the call is dropped — precisely the near-miss this exists to
/// rescue. A call matching no candidate is dropped rather than forwarded.
fn resolve_against_known(
    call: ExtractedToolCall,
    known_tools: &[String],
) -> Option<ExtractedToolCall> {
    if known_tools.is_empty() || known_tools.contains(&call.name) {
        return Some(call);
    }
    let (canonical, canonical_arguments) = normalize_alias(&call.name, &call.arguments);
    if known_tools.contains(&canonical) {
        return Some(ExtractedToolCall {
            name: canonical,
            arguments: canonical_arguments,
            arguments_unparsed: call.arguments_unparsed,
        });
    }
    let native = legion_registry_name(&call.name)?;
    if !known_tools.iter().any(|known| known == native) {
        return None;
    }
    Some(ExtractedToolCall {
        name: native.to_string(),
        // Start from the canonical arguments, not the raw ones: canonical
        // renaming already covers pairs the native table does not repeat
        // (`line` → `start_line`), and dropping it here would silently hand
        // Legion's `read` an argument it ignores.
        arguments: rename_arguments_for_legion_tool(native, &canonical_arguments),
        arguments_unparsed: call.arguments_unparsed,
    })
}

/// Map a model-written tool name onto Legion's native registry name.
///
/// Covers both the names small models reach for unprompted (`Read`, `bash`,
/// `str_replace`) and SmallCode's canonical vocabulary (`read_file`, `patch`,
/// `find_files`), since either can appear once extraction has run.
fn legion_registry_name(name: &str) -> Option<&'static str> {
    match name {
        "read" | "Read" | "read_file" | "readFile" | "view" | "cat" | "open_file" => Some("read"),
        "grep" | "search" | "rg" | "ripgrep" | "search_files" | "grep_search" => Some("grep"),
        "glob" | "find_files" | "ls" | "LS" | "list_directory" | "list_files" => Some("glob"),
        "outline" | "symbols" | "list_symbols" | "document_symbols" => Some("outline"),
        // Both whole-file writers and targeted-edit aliases map here.
        // `edit-as-proposal` accepts either `replacement` (complete content)
        // or an `old_str`/`new_str` fragment resolved by exact unique match,
        // so a substring edit is expressible without being mistaken for whole
        // content — see `legion_ai::patch` and `rename_arguments_for_legion_tool`.
        "edit-as-proposal" | "write_file" | "write" | "create_file" | "edit" | "Edit"
        | "str_replace" | "str_replace_editor" | "replace" | "patch" | "apply_patch" => {
            Some("edit-as-proposal")
        }
        "terminal-command" | "bash" | "shell" | "run_command" | "run_terminal_cmd" | "cmd"
        | "terminal" | "exec" => Some("terminal-command"),
        _ => None,
    }
}

/// Whether an argument object describes a substring edit rather than a
/// whole-file write.
///
/// Keyed on the arguments rather than the tool name, because the name is the
/// least reliable thing a small model produces: a call carrying `old_string`
/// means fragment intent whatever it called the tool.
fn describes_substring_edit(arguments: &Value) -> bool {
    let Value::Object(object) = arguments else {
        return false;
    };
    // `search`/`replace` is the Aider-style spelling of the same pair.
    ["old_string", "old_str", "oldText", "search"]
        .iter()
        .any(|key| object.contains_key(*key))
}

/// Rename arguments onto the keys a Legion native tool expects.
///
/// Directory-listing forms are a shape change rather than a rename: `ls(path)`
/// becomes `glob(pattern)` over that directory.
fn rename_arguments_for_legion_tool(tool: &str, arguments: &Value) -> Value {
    let Value::Object(object) = arguments else {
        return arguments.clone();
    };
    if tool == "glob"
        && !object.contains_key("pattern")
        && let Some(dir) = object.get("path").and_then(Value::as_str)
    {
        let dir = dir.trim_end_matches('/');
        let dir = if dir.is_empty() { "." } else { dir };
        let mut renamed = Map::new();
        renamed.insert("pattern".to_string(), Value::String(format!("{dir}/*")));
        return Value::Object(renamed);
    }
    // An edit is a *fragment* replacement or a whole-file write, and the two
    // want different keys. Mapping a fragment's `new_string` onto
    // `replacement` would tell Legion the fragment is the file's entire new
    // content — the destructive misreading this split exists to prevent.
    let fragment_edit = tool == "edit-as-proposal" && describes_substring_edit(arguments);

    let mut renamed = Map::new();
    for (key, value) in object {
        let key = match (tool, key.as_str()) {
            ("read" | "outline" | "edit-as-proposal", "file_path" | "filepath" | "filePath") => {
                "path"
            }
            ("grep" | "glob", "query") => "pattern",
            ("terminal-command", "cmd") => "command",
            ("edit-as-proposal", "old_string" | "oldText" | "search") => "old_str",
            ("edit-as-proposal", "new_string" | "newText" | "replace") if fragment_edit => {
                "new_str"
            }
            // Whole-content forms only; `new_string` outside a fragment edit
            // has no `old_str` to anchor to, so it is the full replacement.
            ("edit-as-proposal", "content" | "text" | "new_string") if !fragment_edit => {
                "replacement"
            }
            _ => key.as_str(),
        };
        renamed.insert(key.to_string(), value.clone());
    }
    Value::Object(renamed)
}

fn strip_spans(source: &str, spans: &[(usize, usize)]) -> String {
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for &(start, end) in spans {
        if start >= cursor && end <= source.len() {
            out.push_str(&source[cursor..start]);
            cursor = end;
        }
    }
    out.push_str(&source[cursor.min(source.len())..]);
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Source scanners
// ---------------------------------------------------------------------------

const TAG_PAIRS: &[(&str, &str)] = &[
    ("<tool_call>", "</tool_call>"),
    ("<tool-call>", "</tool-call>"),
];

fn scan_tagged(source: &str) -> Option<ScanResult> {
    let mut calls = Vec::new();
    let mut spans = Vec::new();
    for (open, close) in TAG_PAIRS {
        let mut cursor = 0usize;
        while let Some(rel_open) = source[cursor..].find(open) {
            let start = cursor + rel_open;
            let body_start = start + open.len();
            let Some(rel_close) = source[body_start..].find(close) else {
                break;
            };
            let body_end = body_start + rel_close;
            let end = body_end + close.len();
            // A tag with an unparseable body is still consumed: leaving the
            // raw JSON in the prose would invite the model to "read" its own
            // broken call back as context.
            spans.push((start, end));
            calls.extend(calls_from_text(&source[body_start..body_end]));
            cursor = end;
        }
    }
    spans.sort_unstable();
    ScanResult { calls, spans }.non_empty()
}

const LIQUID_PAIRS: &[(&str, &str)] = &[
    ("<|tool_call_start|>", "<|tool_call_end|>"),
    ("<tool_call_start>", "<tool_call_end>"),
];

fn scan_liquid(source: &str) -> Option<ScanResult> {
    let mut calls = Vec::new();
    let mut spans = Vec::new();
    for (open, close) in LIQUID_PAIRS {
        let mut cursor = 0usize;
        while let Some(rel_open) = source[cursor..].find(open) {
            let start = cursor + rel_open;
            let body_start = start + open.len();
            let Some(rel_close) = source[body_start..].find(close) else {
                break;
            };
            let body_end = body_start + rel_close;
            let end = body_end + close.len();
            spans.push((start, end));
            calls.extend(parse_liquid_body(&source[body_start..body_end]));
            cursor = end;
        }
    }
    spans.sort_unstable();
    ScanResult { calls, spans }.non_empty()
}

fn scan_fenced(source: &str) -> Option<ScanResult> {
    let mut calls = Vec::new();
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel_open) = source[cursor..].find("```") {
        let start = cursor + rel_open;
        let after_ticks = start + 3;
        // The remainder of the opening line is the (optional) language tag.
        let line_end = source[after_ticks..]
            .find('\n')
            .map(|idx| after_ticks + idx + 1)
            .unwrap_or(source.len());
        let Some(rel_close) = source[line_end..].find("```") else {
            break;
        };
        let body_end = line_end + rel_close;
        let end = body_end + 3;
        let language = source[after_ticks..line_end.min(source.len())].trim();
        // Only fences that could plausibly carry a call are consumed; a
        // ```rust block is prose as far as this module is concerned.
        if matches!(language, "" | "json" | "tool_call" | "tool-call") {
            let body_calls = calls_from_text(&source[line_end..body_end]);
            if !body_calls.is_empty() {
                spans.push((start, end));
                calls.extend(body_calls);
            }
        }
        cursor = end;
    }
    ScanResult { calls, spans }.non_empty()
}

/// Recover edits written as SEARCH/REPLACE blocks or diff hunks.
///
/// Only runs when the registry actually offers an edit tool, so a chat about
/// a diff does not become an edit request. The whole message is consumed,
/// because a block-format edit *is* the message.
fn scan_edit_blocks(source: &str, known_tools: &[String]) -> Option<ScanResult> {
    const EDIT_TOOL: &str = "edit-as-proposal";
    if !known_tools.is_empty() && !known_tools.iter().any(|tool| tool == EDIT_TOOL) {
        return None;
    }
    let blocks = crate::patch::parse_edit_blocks(source);
    if blocks.is_empty() {
        return None;
    }
    let calls = blocks
        .into_iter()
        .map(|block| {
            let mut arguments = Map::new();
            arguments.insert("path".to_string(), Value::String(block.path));
            if block.old_str.is_empty() {
                // An empty search half means "create this file with exactly
                // this content" — the whole-content form.
                arguments.insert("replacement".to_string(), Value::String(block.new_str));
            } else {
                arguments.insert("old_str".to_string(), Value::String(block.old_str));
                arguments.insert("new_str".to_string(), Value::String(block.new_str));
            }
            ExtractedToolCall {
                name: EDIT_TOOL.to_string(),
                arguments: Value::Object(arguments),
                arguments_unparsed: None,
            }
        })
        .collect();
    ScanResult {
        calls,
        spans: vec![(0, source.len())],
    }
    .non_empty()
}

fn scan_bare(source: &str) -> Option<ScanResult> {
    // Bare JSON counts only when it is the entire message. Prose wrapped
    // around a JSON blob ("Here is the call: {...}") is the model describing
    // an intent, not issuing one.
    let trimmed = source.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    let value = parse_json_lenient(trimmed)?;
    let mut calls = Vec::new();
    collect_calls(&value, &mut calls);
    if calls.is_empty() {
        return None;
    }
    ScanResult {
        calls,
        spans: vec![(0, source.len())],
    }
    .non_empty()
}

// ---------------------------------------------------------------------------
// JSON shape handling
// ---------------------------------------------------------------------------

/// Parse the JSON in a tag body and collect the calls it encodes.
///
/// A single tag may hold one object, an array, or several objects written one
/// per line — all three appear in real small-model output. Objects
/// concatenated *on the same line* are deliberately not recovered: unlike the
/// line-separated form they have no unambiguous delimiter, and brace-scanning
/// them would mean guessing where one call ends and the next begins.
fn calls_from_text(text: &str) -> Vec<ExtractedToolCall> {
    let mut calls = Vec::new();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return calls;
    }
    if let Some(value) = parse_json_lenient(trimmed) {
        collect_calls(&value, &mut calls);
        return calls;
    }
    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < 2 {
        return calls;
    }
    // Every line must parse; a single bad line means the body is malformed
    // rather than line-separated, and partial recovery would drop a call the
    // model asked for without saying so.
    let mut values = Vec::with_capacity(lines.len());
    for line in lines {
        match parse_json_lenient(line) {
            Some(value) => values.push(value),
            None => return Vec::new(),
        }
    }
    for value in values {
        collect_calls(&value, &mut calls);
    }
    calls
}

/// Strict JSON parse with one narrow repair: trailing commas before `}`/`]`.
///
/// Anything more speculative (single-quoted keys, `True`/`None`, truncated
/// input) is rejected — see the module note on not fabricating calls.
fn parse_json_lenient(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return Some(value);
    }
    let repaired = strip_trailing_commas(text);
    if repaired != text
        && let Ok(value) = serde_json::from_str::<Value>(&repaired)
    {
        return Some(value);
    }
    None
}

/// Remove commas that immediately precede a closing `}` or `]`.
///
/// Copies unchanged input as byte *slices* rather than reconstructing it
/// character by character. Only ASCII commas are ever dropped, so every cut
/// lands on a char boundary and multi-byte text passes through byte-exact —
/// these arguments carry source code and file contents, where silently
/// rewriting `é` into `Ã©` would corrupt an edit proposal.
fn strip_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut copied_to = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
            continue;
        }
        if byte == b',' {
            // Drop the comma when the next non-space byte closes a container.
            let next = bytes[idx + 1..]
                .iter()
                .find(|candidate| !candidate.is_ascii_whitespace());
            if matches!(next, Some(b'}') | Some(b']')) {
                out.push_str(&text[copied_to..idx]);
                copied_to = idx + 1;
            }
        }
    }
    out.push_str(&text[copied_to..]);
    out
}

fn collect_calls(value: &Value, out: &mut Vec<ExtractedToolCall>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_calls(item, out);
            }
        }
        Value::Object(object) => {
            if let Some(call) = call_from_object(object) {
                out.push(call);
            }
        }
        _ => {}
    }
}

/// Map the several call shapes small models emit onto one canonical form.
fn call_from_object(object: &Map<String, Value>) -> Option<ExtractedToolCall> {
    // OpenAI wire shape: {"function": {"name", "arguments": "<json string>"}}
    if let Some(Value::Object(function)) = object.get("function") {
        let name = function.get("name")?.as_str()?.to_string();
        return Some(finish_call(name, function.get("arguments")));
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| object.get("tool").and_then(Value::as_str))?
        .to_string();
    if name.is_empty() {
        return None;
    }
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .or_else(|| object.get("parameters"));
    Some(finish_call(name, arguments))
}

fn finish_call(name: String, arguments: Option<&Value>) -> ExtractedToolCall {
    let mut arguments_unparsed = None;
    let arguments = match arguments {
        // Providers often nest the argument object as a JSON *string*.
        Some(Value::String(raw)) => match parse_json_lenient(raw) {
            Some(value) => value,
            None => {
                arguments_unparsed = Some(raw.clone());
                Value::Null
            }
        },
        Some(value) => value.clone(),
        None => Value::Null,
    };
    // Extraction reports the name the model actually wrote. Canonicalization
    // happens only if that name does not match an offered tool — see
    // `resolve_against_known`.
    ExtractedToolCall {
        name,
        arguments,
        arguments_unparsed,
    }
}

// ---------------------------------------------------------------------------
// Liquid (`<|tool_call_start|>[fn(k='v')]<|tool_call_end|>`) parsing
// ---------------------------------------------------------------------------

fn parse_liquid_body(body: &str) -> Vec<ExtractedToolCall> {
    let trimmed = body.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(trimmed);
    let mut parser = LiquidParser::new(inner);
    parser.parse_calls()
}

struct LiquidParser<'a> {
    chars: Vec<char>,
    pos: usize,
    _source: &'a str,
}

impl<'a> LiquidParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            _source: source,
        }
    }

    fn parse_calls(&mut self) -> Vec<ExtractedToolCall> {
        let mut calls = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.chars.len() {
                break;
            }
            match self.parse_call() {
                Some(call) => calls.push(call),
                // A malformed entry aborts the rest of the list: positions
                // after the failure can no longer be trusted.
                None => break,
            }
            self.skip_ws();
            if self.peek() == Some(',') {
                self.pos += 1;
            }
        }
        calls
    }

    fn parse_call(&mut self) -> Option<ExtractedToolCall> {
        let name = self.parse_identifier()?;
        self.skip_ws();
        if self.peek() != Some('(') {
            return None;
        }
        self.pos += 1;
        let mut arguments = Map::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => {
                    self.pos += 1;
                    break;
                }
                None => return None,
                _ => {}
            }
            let key = self.parse_identifier()?;
            self.skip_ws();
            if self.peek() != Some('=') {
                return None;
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.parse_value()?;
            arguments.insert(key, value);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.pos += 1;
            }
        }
        Some(ExtractedToolCall {
            name,
            arguments: Value::Object(arguments),
            // Liquid arguments are parsed structurally: a malformed one aborts
            // the whole call rather than surviving as raw text.
            arguments_unparsed: None,
        })
    }

    fn parse_identifier(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return None;
        }
        Some(self.chars[start..self.pos].iter().collect())
    }

    fn parse_value(&mut self) -> Option<Value> {
        self.skip_ws();
        match self.peek()? {
            '\'' | '"' => self.parse_string().map(Value::String),
            '[' => self.parse_list(),
            '{' => self.parse_dict(),
            _ => self.parse_scalar(),
        }
    }

    fn parse_string(&mut self) -> Option<String> {
        let quote = self.peek()?;
        self.pos += 1;
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            if ch == quote {
                return Some(out);
            }
            if ch != '\\' {
                out.push(ch);
                continue;
            }
            let Some(escape) = self.peek() else {
                // Trailing backslash at end of input: unterminated.
                return None;
            };
            self.pos += 1;
            match escape {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '0' => out.push('\0'),
                '\\' => out.push('\\'),
                '\'' => out.push('\''),
                '"' => out.push('"'),
                'x' => match self.parse_hex(2) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push('\\');
                        out.push('x');
                    }
                },
                'u' => match self.parse_hex(4) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push('\\');
                        out.push('u');
                    }
                },
                // Unknown escapes survive verbatim — `\q` in a shell command
                // or regex is far likelier to be intentional than a typo.
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }
        None
    }

    fn parse_hex(&mut self, width: usize) -> Option<char> {
        if self.pos + width > self.chars.len() {
            return None;
        }
        let digits: String = self.chars[self.pos..self.pos + width].iter().collect();
        let code = u32::from_str_radix(&digits, 16).ok()?;
        let ch = char::from_u32(code)?;
        self.pos += width;
        Some(ch)
    }

    fn parse_list(&mut self) -> Option<Value> {
        self.pos += 1; // consume '['
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                None => return None,
                _ => {}
            }
            items.push(self.parse_value()?);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.pos += 1;
            }
        }
        Some(Value::Array(items))
    }

    fn parse_dict(&mut self) -> Option<Value> {
        self.pos += 1; // consume '{'
        let mut object = Map::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                None => return None,
                _ => {}
            }
            let key = match self.peek()? {
                '\'' | '"' => self.parse_string()?,
                _ => self.parse_identifier()?,
            };
            self.skip_ws();
            if self.peek() != Some(':') {
                return None;
            }
            self.pos += 1;
            let value = self.parse_value()?;
            object.insert(key, value);
            self.skip_ws();
            if self.peek() == Some(',') {
                self.pos += 1;
            }
        }
        Some(Value::Object(object))
    }

    fn parse_scalar(&mut self) -> Option<Value> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == ',' || ch == ')' || ch == ']' || ch == '}' {
                break;
            }
            self.pos += 1;
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        Some(match raw {
            "True" | "true" => Value::Bool(true),
            "False" | "false" => Value::Bool(false),
            "None" | "null" => Value::Null,
            _ => match raw.parse::<i64>() {
                Ok(int) => Value::from(int),
                Err(_) => match raw.parse::<f64>() {
                    Ok(float) => Value::from(float),
                    Err(_) => Value::String(raw.to_string()),
                },
            },
        })
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Alias normalization
// ---------------------------------------------------------------------------

/// Canonical tool name for a near-miss alias, plus the argument renames that
/// canonical tool expects. Unknown names pass through untouched.
pub fn normalize_alias(name: &str, arguments: &Value) -> (String, Value) {
    let canonical = canonical_tool_name(name);
    let Some(canonical) = canonical else {
        return (name.to_string(), arguments.clone());
    };
    // Directory-listing aliases are a shape change, not a rename: `ls(path)`
    // becomes `find_files(pattern)` over that directory.
    if canonical == "find_files" && is_directory_listing_alias(name) {
        let dir = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .trim_end_matches('/');
        let dir = if dir.is_empty() { "." } else { dir };
        let mut object = Map::new();
        object.insert("pattern".to_string(), Value::String(format!("{dir}/*")));
        return (canonical.to_string(), Value::Object(object));
    }
    let Value::Object(object) = arguments else {
        // Arguments that never parsed: rename the tool, leave them unresolved
        // so the call cannot be dispatched with invented data.
        return (canonical.to_string(), arguments.clone());
    };
    let mut renamed = Map::new();
    for (key, value) in object {
        let key = canonical_argument_name(canonical, key);
        renamed.insert(key.to_string(), value.clone());
    }
    (canonical.to_string(), Value::Object(renamed))
}

fn canonical_tool_name(name: &str) -> Option<&'static str> {
    match name {
        "Read" | "read" | "view" | "read_file" => Some("read_file"),
        "Edit" | "edit" | "str_replace" | "str_replace_editor" | "replace" | "patch" => {
            Some("patch")
        }
        "shell" | "run_command" | "bash" => Some("bash"),
        "grep" | "search" => Some("search"),
        "glob" | "LS" | "ls" | "list_directory" | "find_files" => Some("find_files"),
        _ => None,
    }
}

fn is_directory_listing_alias(name: &str) -> bool {
    matches!(name, "LS" | "ls" | "list_directory")
}

fn canonical_argument_name<'a>(tool: &str, key: &'a str) -> &'a str {
    match (tool, key) {
        ("read_file", "file_path" | "filepath") => "path",
        ("read_file", "line") => "start_line",
        ("patch", "file_path" | "filepath") => "path",
        ("patch", "old_string") => "old_str",
        ("patch", "new_string") => "new_str",
        ("bash", "cmd") => "command",
        ("search", "query") => "pattern",
        ("find_files", "query") => "pattern",
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(content: &str, known: &[&str]) -> ToolCallExtraction {
        let known: Vec<String> = known.iter().map(|s| s.to_string()).collect();
        extract_tool_calls(&ExtractionInput {
            content,
            reasoning_content: None,
            has_existing_tool_calls: false,
            known_tools: &known,
        })
    }

    #[test]
    fn tagged_call_is_recovered_and_removed_from_prose() {
        let out = extract(
            "I will list the files now.\n<tool_call>{\"name\":\"bash\",\"arguments\":{\"command\":\"ls\"}}</tool_call>\nDone.",
            &["bash"],
        );
        assert_eq!(out.calls.len(), 1);
        assert_eq!(out.calls[0].name, "bash");
        assert_eq!(out.residual_content, "I will list the files now.\n\nDone.");
    }

    #[test]
    fn tagged_source_wins_over_fenced_restatement() {
        let out = extract(
            "<tool_call>{\"name\":\"bash\",\"arguments\":{\"command\":\"ls\"}}</tool_call>\n```json\n{\"name\":\"read_file\",\"arguments\":{\"path\":\"a.py\"}}\n```",
            &["bash", "read_file"],
        );
        assert_eq!(out.calls.len(), 1, "only the tagged call is consumed");
        assert_eq!(out.calls[0].name, "bash");
    }

    #[test]
    fn bare_json_requires_the_whole_message() {
        let described = extract(
            "Here is the call: {\"name\": \"bash\", \"arguments\": {\"command\": \"ls\"}}",
            &["bash"],
        );
        assert!(
            described.calls.is_empty(),
            "prose describing a call is not a call"
        );
        let issued = extract(
            "{\"name\": \"bash\", \"arguments\": {\"command\": \"ls\"}}",
            &["bash"],
        );
        assert_eq!(issued.calls.len(), 1);
    }

    #[test]
    fn unrepairable_json_yields_no_call() {
        for raw in [
            "<tool_call>{'name': 'bash', 'arguments': {'command': 'ls'}}</tool_call>",
            "<tool_call>{\"name\": \"search\", \"arguments\": {\"regex\": True}}</tool_call>",
            "<tool_call>{\"name\": \"write_file\", \"arguments\": {\"content\": \"print(",
        ] {
            let out = extract(raw, &["bash", "search", "write_file"]);
            assert!(
                out.calls.is_empty(),
                "must not fabricate a call from: {raw}"
            );
        }
    }

    #[test]
    fn trailing_commas_are_repaired() {
        let out = extract(
            "<tool_call>{\"name\":\"bash\",\"arguments\":{\"command\":\"ls -la\",},}</tool_call>",
            &["bash"],
        );
        assert_eq!(out.calls.len(), 1);
        assert_eq!(out.calls[0].arguments["command"], "ls -la");
    }

    /// Legion's *real* delegated-loop registry, not a SmallCode-shaped one.
    /// Testing against invented names would let the alias layer look healthy
    /// while dropping every call in production.
    const LEGION_TOOLS: &[&str] = &[
        "read",
        "grep",
        "glob",
        "outline",
        "edit-as-proposal",
        "terminal-command",
    ];

    #[test]
    fn near_miss_names_resolve_to_legion_registry_names() {
        for (written, expected_tool, expected_key, expected_value) in [
            (
                "<tool_call>{\"name\":\"Read\",\"arguments\":{\"file_path\":\"src/foo.rs\"}}</tool_call>",
                "read",
                "path",
                "src/foo.rs",
            ),
            (
                "<tool_call>{\"name\":\"read_file\",\"arguments\":{\"path\":\"a.rs\"}}</tool_call>",
                "read",
                "path",
                "a.rs",
            ),
            (
                "<tool_call>{\"name\":\"search\",\"arguments\":{\"query\":\"TODO\"}}</tool_call>",
                "grep",
                "pattern",
                "TODO",
            ),
            (
                "<tool_call>{\"name\":\"shell\",\"arguments\":{\"cmd\":\"ls\"}}</tool_call>",
                "terminal-command",
                "command",
                "ls",
            ),
            (
                "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"file_path\":\"a.rs\",\"content\":\"whole file\"}}</tool_call>",
                "edit-as-proposal",
                "replacement",
                "whole file",
            ),
        ] {
            let out = extract(written, LEGION_TOOLS);
            assert_eq!(out.calls.len(), 1, "must be recovered: {written}");
            assert_eq!(out.calls[0].name, expected_tool, "for: {written}");
            assert_eq!(
                out.calls[0].arguments[expected_key], expected_value,
                "argument renamed onto the Legion key for: {written}"
            );
        }
    }

    #[test]
    fn substring_edits_keep_fragment_semantics_and_never_become_whole_file_writes() {
        // The failure this guards against: mapping a fragment's `new_string`
        // onto `replacement` tells Legion the fragment is the file's entire
        // new content, deleting everything else.
        for raw in [
            "<tool_call>{\"name\":\"str_replace\",\"arguments\":{\"file_path\":\"a.rs\",\"old_string\":\"foo\",\"new_string\":\"bar\"}}</tool_call>",
            "<tool_call>{\"name\":\"Edit\",\"arguments\":{\"file_path\":\"a.rs\",\"old_string\":\"foo\",\"new_string\":\"bar\"}}</tool_call>",
            "<tool_call>{\"name\":\"patch\",\"arguments\":{\"path\":\"a.rs\",\"old_str\":\"foo\",\"new_str\":\"bar\"}}</tool_call>",
            // Even under a whole-file name, `old_string` means fragment intent.
            "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"a.rs\",\"old_string\":\"foo\",\"new_string\":\"bar\"}}</tool_call>",
        ] {
            let out = extract(raw, LEGION_TOOLS);
            assert_eq!(out.calls.len(), 1, "fragment edit is recovered: {raw}");
            let call = &out.calls[0];
            assert_eq!(call.name, "edit-as-proposal");
            assert_eq!(call.arguments["old_str"], "foo", "for: {raw}");
            assert_eq!(call.arguments["new_str"], "bar", "for: {raw}");
            assert!(
                call.arguments.get("replacement").is_none(),
                "a fragment must never be forwarded as whole-file content: {raw}"
            );
        }
    }

    #[test]
    fn edits_written_as_blocks_become_edit_calls() {
        let out = extract(
            "Here is the change:\nsrc/lib.rs\n<<<<<<< SEARCH\nfn old_name() {}\n=======\nfn new_name() {}\n>>>>>>> REPLACE\n",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls.len(), 1, "a block-format edit is recovered");
        assert_eq!(out.calls[0].name, "edit-as-proposal");
        assert_eq!(out.calls[0].arguments["path"], "src/lib.rs");
        assert_eq!(out.calls[0].arguments["old_str"], "fn old_name() {}");
        assert_eq!(out.calls[0].arguments["new_str"], "fn new_name() {}");
    }

    #[test]
    fn an_empty_search_half_becomes_a_whole_file_write() {
        let out = extract(
            "src/new.rs\n<<<<<<< SEARCH\n=======\npub fn hello() {}\n>>>>>>> REPLACE\n",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls[0].arguments["replacement"], "pub fn hello() {}");
        assert!(out.calls[0].arguments.get("old_str").is_none());
    }

    #[test]
    fn block_recovery_needs_an_edit_tool_on_offer() {
        // Discussing a diff is not requesting an edit.
        let out = extract(
            "src/lib.rs\n<<<<<<< SEARCH\nfn a() {}\n=======\nfn b() {}\n>>>>>>> REPLACE\n",
            &["read", "grep"],
        );
        assert!(out.calls.is_empty());
    }

    #[test]
    fn a_real_tool_call_still_wins_over_a_block_restatement() {
        let out = extract(
            "<tool_call>{\"name\":\"read\",\"arguments\":{\"path\":\"a.rs\"}}</tool_call>\nsrc/lib.rs\n<<<<<<< SEARCH\nfn a() {}\n=======\nfn b() {}\n>>>>>>> REPLACE\n",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls.len(), 1);
        assert_eq!(out.calls[0].name, "read");
    }

    #[test]
    fn whole_file_writes_still_map_to_replacement() {
        let out = extract(
            "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"path\":\"a.rs\",\"content\":\"whole file\"}}</tool_call>",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls[0].arguments["replacement"], "whole file");
        assert!(out.calls[0].arguments.get("new_str").is_none());
    }

    #[test]
    fn canonical_argument_renames_survive_native_resolution() {
        // `line` → `start_line` comes from the canonical table, not the native
        // one; losing it would make `read` return the whole file.
        let out = extract(
            "<tool_call>{\"name\":\"Read\",\"arguments\":{\"file_path\":\"a.rs\",\"line\":200}}</tool_call>",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls[0].name, "read");
        assert_eq!(out.calls[0].arguments["path"], "a.rs");
        assert_eq!(
            out.calls[0].arguments["start_line"], 200,
            "the canonical rename must reach the native call"
        );
    }

    #[test]
    fn directory_listing_names_become_glob_patterns_for_legion() {
        let out = extract(
            "<tool_call>{\"name\":\"ls\",\"arguments\":{\"path\":\"src/\"}}</tool_call>",
            LEGION_TOOLS,
        );
        assert_eq!(out.calls[0].name, "glob");
        assert_eq!(out.calls[0].arguments["pattern"], "src/*");
    }

    #[test]
    fn a_smallcode_shaped_registry_still_resolves_to_its_own_names() {
        // Registries using SmallCode's vocabulary keep working.
        let out = extract(
            "<tool_call>{\"name\":\"Read\",\"arguments\":{\"file_path\":\"src/foo.rs\"}}</tool_call>",
            &["read_file", "bash"],
        );
        assert_eq!(out.calls[0].name, "read_file");
        assert_eq!(out.calls[0].arguments["path"], "src/foo.rs");
    }

    #[test]
    fn a_literal_tool_name_is_never_rewritten_by_an_alias() {
        // `shell` is a real tool here, so it must stay `shell` even though it
        // is also an alias for `bash`.
        let out = extract(
            "<tool_call>{\"name\":\"shell\",\"arguments\":{\"cmd\":\"ls\"}}</tool_call>",
            &["shell", "read_file"],
        );
        assert_eq!(out.calls[0].name, "shell");
        assert_eq!(out.calls[0].arguments["cmd"], "ls");
    }

    #[test]
    fn unparseable_nested_arguments_are_flagged_not_silently_nulled() {
        let out = extract(
            "<tool_call>{\"function\":{\"name\":\"bash\",\"arguments\":\"{bad json\"}}</tool_call>",
            &["bash"],
        );
        assert_eq!(out.calls.len(), 1);
        assert_eq!(
            out.calls[0].arguments_unparsed.as_deref(),
            Some("{bad json"),
            "callers must be able to tell this apart from a call with no arguments"
        );

        let absent = extract("<tool_call>{\"name\":\"bash\"}</tool_call>", &["bash"]);
        assert!(
            absent.calls[0].arguments_unparsed.is_none(),
            "an absent argument object is not a parse failure"
        );
    }

    #[test]
    fn trailing_comma_repair_preserves_multibyte_text() {
        // Repair rewrites the argument text, so a byte-wise rebuild would
        // corrupt any non-ASCII content it passes through — and this content
        // becomes edit proposals.
        let out = extract(
            "<tool_call>{\"name\":\"write_file\",\"arguments\":{\"content\":\"café 🌍 日本語\",}}</tool_call>",
            &["write_file"],
        );
        assert_eq!(out.calls.len(), 1);
        assert_eq!(
            out.calls[0].arguments["content"], "café 🌍 日本語",
            "multi-byte characters must survive trailing-comma repair byte-exact"
        );
    }

    #[test]
    fn unknown_tools_are_dropped() {
        let out = extract(
            "<tool_call>{\"name\":\"launch_rocket\",\"arguments\":{}}</tool_call>",
            &["read_file", "bash"],
        );
        assert!(out.calls.is_empty());
    }

    #[test]
    fn existing_structured_calls_suppress_recovery() {
        let known = vec!["bash".to_string()];
        let out = extract_tool_calls(&ExtractionInput {
            content: "<tool_call>{\"name\":\"bash\",\"arguments\":{\"command\":\"ls\"}}</tool_call>",
            reasoning_content: None,
            has_existing_tool_calls: true,
            known_tools: &known,
        });
        assert!(out.calls.is_empty(), "never double-count a structured call");
    }

    #[test]
    fn reasoning_channel_is_used_only_when_content_is_blank() {
        let known = vec!["write_file".to_string()];
        let out = extract_tool_calls(&ExtractionInput {
            content: "",
            reasoning_content: Some(
                "<|tool_call_start|>[write_file(path='Makefile', content='build:\\n\\t@echo building')]<|tool_call_end|>",
            ),
            has_existing_tool_calls: false,
            known_tools: &known,
        });
        assert_eq!(out.calls.len(), 1);
        assert_eq!(
            out.calls[0].arguments["content"],
            "build:\n\t@echo building"
        );
    }

    #[test]
    fn liquid_escapes_follow_python_semantics() {
        let out = extract(
            "<|tool_call_start|>[write_file(path='x.txt', content='A\\x41\\u00e9')]<|tool_call_end|>",
            &["write_file"],
        );
        assert_eq!(out.calls[0].arguments["content"], "AA\u{e9}");

        let unknown = extract(
            "<|tool_call_start|>[write_file(path='x.txt', content='a\\qb')]<|tool_call_end|>",
            &["write_file"],
        );
        assert_eq!(
            unknown.calls[0].arguments["content"], "a\\qb",
            "unknown escapes survive verbatim"
        );
    }

    #[test]
    fn openai_function_shape_parses_stringified_arguments() {
        let out = extract(
            "<tool_call>{\"function\": {\"name\": \"bash\", \"arguments\": \"{\\\"command\\\": \\\"pwd\\\"}\"}}</tool_call>",
            &["bash"],
        );
        assert_eq!(out.calls[0].name, "bash");
        assert_eq!(out.calls[0].arguments["command"], "pwd");
    }

    #[test]
    fn directory_listing_aliases_become_glob_patterns() {
        let (name, args) = normalize_alias("ls", &serde_json::json!({"path": "src/"}));
        assert_eq!(name, "find_files");
        assert_eq!(args["pattern"], "src/*");

        let (_, defaulted) = normalize_alias("ls", &Value::Null);
        assert_eq!(defaulted["pattern"], "./*");
    }

    #[test]
    fn alias_with_unparseable_arguments_renames_without_inventing_data() {
        let (name, args) = normalize_alias("Edit", &Value::Null);
        assert_eq!(name, "patch");
        assert!(
            args.is_null(),
            "arguments stay unresolved so the call cannot be dispatched"
        );
    }
}
