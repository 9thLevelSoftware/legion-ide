//! Shared helpers for desktop integration tests.
//!
//! These utilities back the architectural-boundary tests that assert a module's
//! source does (or does not) reference a given symbol. Raw `str::contains`
//! checks are brittle: they match inside doc comments, string literals, and as
//! substrings of unrelated identifiers (e.g. `legion_app` inside
//! `legion_application`, or `EditorEngine` inside `EditorEngineProxy`). The
//! helpers here strip comments and string/raw-string literals first, then match
//! on whole identifier tokens only, so the boundary checks fail only on a real
//! source reference.

// Each test binary that includes this module only uses a subset of the helpers,
// so suppress the per-binary dead-code warnings the shared-module pattern emits.
#![allow(dead_code)]

/// Returns `source` with line comments, block comments (nested), and
/// double-quoted/raw string literals replaced by spaces. Char literals and
/// lifetimes are left intact (they cannot contain the multi-character
/// identifiers these tests scan for, so they never produce false positives).
pub fn strip_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();

        // Line comment: // ... \n
        if b == b'/' && next == Some(b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }

        // Block comment (supports nesting): /* ... */
        if b == b'/' && next == Some(b'*') {
            let mut depth = 1;
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
            }
            continue;
        }

        // Raw string literal: r"...", r#"..."#, r##"..."##, ...
        if b == b'r' && matches!(next, Some(b'"') | Some(b'#')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < bytes.len() && bytes[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                // Confirmed raw string opener.
                out.push(' '); // r
                for _ in 0..hashes {
                    out.push(' ');
                }
                out.push(' '); // opening quote
                j += 1;
                loop {
                    if j >= bytes.len() {
                        break;
                    }
                    if bytes[j] == b'"' {
                        let mut k = j + 1;
                        let mut closing = 0;
                        while k < bytes.len() && bytes[k] == b'#' && closing < hashes {
                            closing += 1;
                            k += 1;
                        }
                        if closing == hashes {
                            out.push(' '); // closing quote
                            for _ in 0..hashes {
                                out.push(' ');
                            }
                            j = k;
                            break;
                        }
                    }
                    if bytes[j] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            // Not a raw string (e.g. an identifier starting with `r`): fall through.
        }

        // Regular string literal: "..." with \" escapes.
        if b == b'"' {
            out.push(' ');
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    out.push(' ');
                    if i + 1 < bytes.len() {
                        out.push(' ');
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    out.push(' ');
                    i += 1;
                    break;
                }
                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            continue;
        }

        // Default: copy the byte. Source is UTF-8; multibyte chars are copied
        // byte-for-byte which is safe because we only compare ASCII identifiers.
        out.push(b as char);
        i += 1;
    }
    out
}

fn is_ident_byte(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

/// True if `ident` appears in `source` as a whole identifier token, ignoring
/// occurrences inside comments and string literals.
pub fn source_uses_identifier(source: &str, ident: &str) -> bool {
    assert!(!ident.is_empty(), "identifier must not be empty");
    let stripped = strip_comments_and_strings(source);
    let hay = stripped.as_bytes();
    let needle = ident.as_bytes();
    let mut start = 0;
    while let Some(pos) = find_from(hay, needle, start) {
        let before_ok = pos == 0 || !is_ident_byte(hay[pos - 1]);
        let after_idx = pos + needle.len();
        let after_ok = after_idx >= hay.len() || !is_ident_byte(hay[after_idx]);
        if before_ok && after_ok {
            return true;
        }
        start = pos + 1;
    }
    false
}

fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    let mut i = from;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Asserts the given source references none of the forbidden identifiers (as
/// whole tokens), reporting the offending symbol on failure.
pub fn assert_source_excludes(source: &str, label: &str, forbidden: &[&str]) {
    for symbol in forbidden {
        assert!(
            !source_uses_identifier(source, symbol),
            "{label} must not reference `{symbol}` (architectural boundary)"
        );
    }
}

/// Asserts the given source references the identifier (as a whole token).
pub fn assert_source_includes(source: &str, label: &str, symbol: &str) {
    assert!(
        source_uses_identifier(source, symbol),
        "{label} should reference `{symbol}`"
    );
}

/// A throwaway workspace directory for a single test.
///
/// Two rules this consolidates, both of which had already started to drift
/// between copies: the directory name must be unique across concurrently
/// running tests, and `Drop` must refuse to delete anything it did not create.
///
/// The uniqueness components are separate path segments, never summed — adding
/// a counter to a millisecond timestamp lets two roots collide whenever
/// `millis_a + counter_a == millis_b + counter_b`, which is the bug removed
/// from fifteen harnesses in `legion-app` and `legion-project`.
pub struct TempWorkspace {
    root: std::path::PathBuf,
    prefix: &'static str,
}

static TEMP_WORKSPACE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl TempWorkspace {
    /// Create a workspace directory named after `prefix`.
    pub fn new(prefix: &'static str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let id = TEMP_WORKSPACE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("{prefix}_{}_{nanos}_{id}", std::process::id()));
        // `create_dir`, not `create_dir_all`: a name collision must fail here
        // rather than silently hand two tests the same workspace.
        std::fs::create_dir(&root).expect("temp workspace should be created");
        Self { root, prefix }
    }

    /// The workspace root.
    pub fn path(&self) -> &std::path::Path {
        &self.root
    }

    /// Write a file inside the workspace, creating parent directories.
    pub fn write(&self, relative: &str, contents: &str) -> std::path::PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directory should be created");
        }
        std::fs::write(&path, contents).expect("temp file should be written");
        path
    }

    /// Create a directory inside the workspace.
    pub fn mkdir(&self, relative: &str) -> std::path::PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        // Guarded on both the system temp root and the prefix this instance was
        // created with, so a bug in path construction cannot turn cleanup into
        // deleting something that matters.
        let temp_root = std::env::temp_dir();
        let named_by_us = self
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(self.prefix));
        if self.root.starts_with(&temp_root) && named_by_us {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

// --- Rendered-UI driving ---------------------------------------------------
//
// Tests that assert on rendering and hit-testing need to click real controls at
// real coordinates, which projection tests cannot do by construction. These
// four helpers are the whole rig, and they live here because three test files
// had grown near-identical copies — two of which disagreed about what happens
// when the control is missing.
//
// `shell_affordances.rs` deliberately keeps its own. Its `clickable_center`
// asserts there is *exactly one* match inside a screen region, which catches a
// label that appears in both the tab strip and an explorer row; that is a
// stronger contract than "find the first one", not a copy of it, and its
// viewport differs too. Folding it in here would weaken the check it exists to
// make.

/// A full-frame raw input at a fixed 1440x900 viewport.
///
/// Fixed rather than parameterised so a control's coordinates are reproducible
/// across suites; a test that needs a different viewport should say so by
/// building its own.
pub fn full_frame_input(events: Vec<egui::Event>) -> egui::RawInput {
    // Carry the chord's modifiers on the frame as well as on the event.
    //
    // egui answers `input.modifiers` from `RawInput::modifiers`, not from
    // whichever event happens to be in the queue, and the central keybinding
    // dispatcher tests `input.modifiers.command`. Leaving this at the default
    // made every modifier chord sent through this helper arrive as a bare
    // keypress -- so Ctrl+S dispatched nothing, and a test written to check
    // that Ctrl+S saves would have reported the product broken when the harness
    // was. Taken from the last pressed key event, which is the chord the frame
    // is being built for.
    let modifiers = events
        .iter()
        .rev()
        .find_map(|event| match event {
            egui::Event::Key {
                pressed: true,
                modifiers,
                ..
            } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    egui::RawInput {
        focused: true,
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1_440.0, 900.0),
        )),
        modifiers,
        events,
        ..egui::RawInput::default()
    }
}

/// Centre of the clickable accessibility node carrying `label`, if one exists.
///
/// Returns `Option` rather than panicking: "is this control present at all" is
/// a question some tests exist to answer, and a helper that dies on absence
/// cannot be used to ask it. Callers that require the control write
/// `.unwrap_or_else(|| panic!(...))` at the call site, where the message can say
/// what the test was doing.
/// The accessibility *description* of the node carrying `label`.
///
/// egui exposes a control's name as its label and its supplementary state as a
/// description -- which is where the editor tab puts "Unsaved changes". A test
/// reading only labels cannot see it, and neither can anything else that reads
/// the tree by name alone.
pub fn node_description(output: &egui::FullOutput, label: &str) -> Option<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find(|(_id, node)| node.label() == Some(label))
        .and_then(|(_id, node)| node.description().map(str::to_string))
}

pub fn clickable_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    output
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find_map(|(_id, node)| {
            (node.label() == Some(label) && node.supports_action(egui::accesskit::Action::Click))
                .then(|| node.bounds())
                .flatten()
        })
        .map(|bounds| {
            egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            )
        })
}

/// Every piece of text a frame exposes to assistive technology.
///
/// Reads `label` **or** `value`. egui puts a control's explicit label in the
/// first and static text in the second, so a label-only reader sees buttons and
/// misses every heading, hint and empty state.
pub fn rendered_text(output: &egui::FullOutput) -> Vec<String> {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .map(|update| {
            update
                .nodes
                .iter()
                .filter_map(|(_id, node)| {
                    node.label()
                        .map(str::to_string)
                        .or_else(|| node.value().map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A full-frame raw input carrying one key press.
///
/// `modifiers` is set on the event **and** on the frame. egui reads held
/// modifiers from `RawInput::modifiers`, not from the event they arrive with,
/// so a helper that sets only the event's copy sends `Shift+F5` and has it
/// dispatch as a plain `F5`. That failure is silent in exactly the case that
/// matters: `F5` (continue) and `Shift+F5` (stop) both move the session, so the
/// test sees state change and calls the wrong binding a pass.
pub fn key_press_input(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
    let mut input = full_frame_input(vec![egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }]);
    input.modifiers = modifiers;
    input
}

/// Press `key` and settle: the key frame, then one frame for the queued action.
pub fn press_key(
    app: &mut legion_desktop::workflow::DesktopEframeApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    let _ = app.run_headless_full_frame(key_press_input(key, modifiers));
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}

/// Click at `pos` and settle: press, release, then one frame for the action.
///
/// Three frames because the action a click queues is applied on the frame after
/// it is dispatched; asserting on the release frame reads the state before the
/// click took effect.
pub fn click_at(
    app: &mut legion_desktop::workflow::DesktopEframeApp,
    pos: egui::Pos2,
) -> egui::FullOutput {
    for pressed in [true, false] {
        let _ = app.run_headless_full_frame(full_frame_input(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            },
        ]));
    }
    app.run_headless_full_frame(full_frame_input(Vec::new()))
}
