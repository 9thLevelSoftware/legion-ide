//! Schema-constrained tool calling for models that cannot emit tool calls.
//!
//! Measured on both local models in the bench corpus
//! (`plans/evidence/production/BENCH/baseline-raw-v1.md`): qwen2.5-coder at 7B
//! and 14B never return `tool_calls`. They write the call as JSON in the
//! message content and report `finish_reason: "stop"`, so a strict provider
//! sees prose and an ended turn. Their *content* is largely right — the 14B
//! names the tool and its arguments correctly — so the failure is transport,
//! not comprehension.
//!
//! [`normalize`](crate::normalize) recovers those calls after the fact. This
//! module removes the need to: instead of advertising `tools` and hoping, the
//! request carries a JSON Schema the decoder must satisfy, so a malformed call
//! is not something to repair but something the model could not have produced.
//! On the 7B that structurally eliminates its most common edit failure — a
//! `new_str` with no `old_str` — because the schema makes both mandatory.
//!
//! The two layers are complements, not alternatives. Constrained decoding is
//! only available on endpoints that support it; recovery still covers
//! everything else, and covers a model that ignores its grammar.
//!
//! **This does not widen authority.** The model chooses among exactly the
//! tools it was already offered, and a request is still only a request:
//! authorization stays with the capability broker and mutations stay
//! proposal-mediated (ADR-0049).

use serde_json::{Map, Value, json};

/// The name reported for the schema in an OpenAI-compatible request.
pub const ACTION_SCHEMA_NAME: &str = "legion_action";

/// The synthetic variant a model picks to end its turn.
///
/// Needed because a constrained response cannot also be free text: with the
/// grammar applied the model can only emit the object, so "I am finished" has
/// to be expressible *within* the schema or the loop could never terminate.
pub const DONE_TOOL: &str = "done";

/// One tool the model may choose, as the loop already knows it.
#[derive(Debug, Clone)]
pub struct SchemaTool {
    /// Tool name, exactly as the executor expects it.
    pub name: String,
    /// The tool's JSON Schema for its arguments.
    pub parameters: Value,
}

/// Build the union schema describing every action the model may take.
///
/// One `anyOf` branch per tool, each pinning `tool` to a constant so the
/// discriminator cannot disagree with the arguments, plus a `done` branch.
/// Returns `None` when there are no tools — an empty union matches nothing and
/// would leave the decoder with no legal output at all.
pub fn build_action_schema(tools: &[SchemaTool]) -> Option<Value> {
    if tools.is_empty() {
        return None;
    }
    let mut branches: Vec<Value> = Vec::with_capacity(tools.len() + 1);
    for tool in tools {
        branches.push(tool_branch(tool));
    }
    branches.push(json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            THOUGHT_FIELD: {"type": "string"},
            "tool": {"const": DONE_TOOL},
            "summary": {"type": "string"}
        },
        "required": [THOUGHT_FIELD, "tool", "summary"]
    }));
    Some(json!({ "anyOf": branches }))
}

/// The field a constrained model reasons in before committing to an action.
///
/// A grammar leaves no room for thinking aloud: the model may emit the action
/// object and nothing else, so the reasoning it would normally write before a
/// tool call has nowhere to go. Measured on qwen2.5-coder:14b, that is not
/// free — a constrained run without this field edited the right files on 6 of
/// 13 tasks and got every single edit wrong, against a recovery-based run
/// making fewer but better edits.
///
/// **The leading underscore is load-bearing.** Grammar-constrained decoders
/// emit object properties in the order the schema lists them, and
/// `serde_json` serializes maps alphabetically unless built with
/// `preserve_order` — which this workspace is not, and enabling it would
/// change key ordering in every serialized structure, not just this one. Named
/// `thought` the field sorts after `new_str`, `old_str` and `path`, so the
/// model would emit its reasoning *after* the arguments that reasoning is
/// supposed to inform: a justification of tokens already chosen. `_` (0x5F)
/// sorts before every lowercase letter, so `_thought` is generated first.
pub const THOUGHT_FIELD: &str = "_thought";

/// One `anyOf` branch: the tool's own schema with a `tool` discriminator.
fn tool_branch(tool: &SchemaTool) -> Value {
    let mut properties = Map::new();
    properties.insert(
        THOUGHT_FIELD.to_string(),
        json!({
            "type": "string",
            "description": "Reason about the task and this specific action before choosing its arguments."
        }),
    );
    properties.insert("tool".to_string(), json!({"const": tool.name}));

    let mut required = vec![json!(THOUGHT_FIELD), json!("tool")];
    if let Some(Value::Object(props)) = tool.parameters.get("properties") {
        for (key, spec) in props {
            properties.insert(key.clone(), strip_nullable(spec));
        }
    }
    if let Some(Value::Array(names)) = tool.parameters.get("required") {
        required.extend(names.iter().cloned());
    }

    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": Value::Object(properties),
        "required": Value::Array(required)
    })
}

/// Collapse `["string", "null"]` to `"string"`.
///
/// Legion's tool schemas mark optional arguments nullable so a model may pass
/// an explicit null. Under a grammar that is worse than useless: it lets the
/// decoder satisfy `old_str` by emitting `null`, which is exactly the omission
/// the constraint exists to prevent. An argument that is genuinely optional is
/// expressed by leaving it out of `required`, which the grammar honours.
fn strip_nullable(spec: &Value) -> Value {
    let Some(object) = spec.as_object() else {
        return spec.clone();
    };
    let mut out = object.clone();
    if let Some(Value::Array(types)) = object.get("type") {
        let concrete: Vec<&Value> = types
            .iter()
            .filter(|t| t.as_str() != Some("null"))
            .collect();
        if let [only] = concrete.as_slice() {
            out.insert("type".to_string(), (*only).clone());
        }
    }
    Value::Object(out)
}

/// One action parsed back out of a constrained response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaAction {
    /// The model chose a tool.
    Call {
        /// Tool name.
        name: String,
        /// Arguments, with the `tool` discriminator removed.
        arguments: Value,
    },
    /// The model ended its turn.
    Done {
        /// Its closing message.
        summary: String,
    },
}

/// Parse a constrained response body into an action.
///
/// Returns `None` for anything that is not a recognisable action object.
/// Deliberately lenient about *surrounding* text — some endpoints prepend
/// whitespace or a stray fence even under a grammar — but strict about the
/// object itself: a missing or non-string `tool` is not an action, and
/// guessing one would fabricate a call the model did not make.
pub fn parse_action(content: &str) -> Option<SchemaAction> {
    let value: Value = serde_json::from_str(content.trim())
        .ok()
        .or_else(|| first_json_object(content))?;
    let object = value.as_object()?;
    let name = object.get("tool")?.as_str()?;
    if name == DONE_TOOL {
        return Some(SchemaAction::Done {
            summary: object
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    let mut arguments = object.clone();
    arguments.remove("tool");
    // The reasoning is for the model, not the executor: forwarding it would
    // fail argument validation on every tool Legion offers. The unprefixed
    // spelling is stripped too — the grammar cannot produce it, but a model
    // ignoring its grammar can, and that should cost a field rather than the
    // whole call.
    arguments.remove(THOUGHT_FIELD);
    arguments.remove("thought");
    Some(SchemaAction::Call {
        name: name.to_string(),
        arguments: Value::Object(arguments),
    })
}

/// Find the first balanced JSON object in `text`.
///
/// Brace counting is string-aware so a `}` inside a value does not end the
/// object early.
fn first_json_object(text: &str) -> Option<Value> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, &byte) in bytes.iter().enumerate().skip(start) {
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
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&text[start..=offset]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit_tool() -> SchemaTool {
        SchemaTool {
            name: "edit-as-proposal".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old_str": {"type": ["string", "null"]},
                    "new_str": {"type": ["string", "null"]}
                },
                "required": ["path"]
            }),
        }
    }

    fn read_tool() -> SchemaTool {
        SchemaTool {
            name: "read".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }),
        }
    }

    #[test]
    fn every_tool_gets_a_branch_plus_done() {
        let schema = build_action_schema(&[read_tool(), edit_tool()]).unwrap();
        let branches = schema["anyOf"].as_array().unwrap();
        assert_eq!(branches.len(), 3, "two tools and a done branch");
        let names: Vec<&str> = branches
            .iter()
            .map(|b| b["properties"]["tool"]["const"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["read", "edit-as-proposal", "done"]);
    }

    #[test]
    fn a_done_branch_exists_so_the_loop_can_terminate() {
        let schema = build_action_schema(&[read_tool()]).unwrap();
        let done = schema["anyOf"].as_array().unwrap().last().unwrap();
        assert_eq!(done["properties"]["tool"]["const"], DONE_TOOL);
        assert!(
            done["required"]
                .as_array()
                .unwrap()
                .contains(&json!("summary")),
            "a constrained response cannot also be free text, so the closing \
             message has to live inside the schema"
        );
    }

    #[test]
    fn no_tools_yields_no_schema() {
        assert!(
            build_action_schema(&[]).is_none(),
            "an empty union matches nothing and would leave the decoder with \
             no legal output at all"
        );
    }

    #[test]
    fn nullable_arguments_become_concrete_types() {
        let schema = build_action_schema(&[edit_tool()]).unwrap();
        let edit = &schema["anyOf"][0];
        assert_eq!(
            edit["properties"]["old_str"]["type"], "string",
            "a nullable type lets the decoder satisfy the field with null, \
             which is the omission the constraint exists to prevent"
        );
    }

    #[test]
    fn the_discriminator_is_always_required() {
        let schema = build_action_schema(&[edit_tool()]).unwrap();
        let required = schema["anyOf"][0]["required"].as_array().unwrap();
        assert!(required.contains(&json!("tool")));
        assert!(required.contains(&json!("path")));
    }

    #[test]
    fn a_tools_own_required_fields_are_preserved() {
        let strict = SchemaTool {
            name: "edit-as-proposal".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old_str": {"type": "string"},
                    "new_str": {"type": "string"}
                },
                "required": ["path", "old_str", "new_str"]
            }),
        };
        let schema = build_action_schema(&[strict]).unwrap();
        let required = schema["anyOf"][0]["required"].as_array().unwrap();
        for field in ["tool", "path", "old_str", "new_str"] {
            assert!(required.contains(&json!(field)), "{field} must be required");
        }
    }

    #[test]
    fn every_branch_makes_room_to_reason_first() {
        let schema = build_action_schema(&[read_tool(), edit_tool()]).unwrap();
        for branch in schema["anyOf"].as_array().unwrap() {
            let props = branch["properties"].as_object().unwrap();
            assert!(
                props.contains_key(THOUGHT_FIELD),
                "a grammar leaves no room for thinking aloud unless the schema \
                 provides it: {branch}"
            );
            assert!(
                branch["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(THOUGHT_FIELD)),
                "optional reasoning is reasoning the model will skip"
            );
            let first = props.keys().next().unwrap();
            assert_eq!(
                first, THOUGHT_FIELD,
                "constrained decoders emit properties in schema order, so the \
                 reasoning has to come before the arguments it informs — \
                 otherwise it is a justification of tokens already chosen"
            );
        }
    }

    #[test]
    fn the_reasoning_never_reaches_the_executor() {
        let action =
            parse_action(r#"{"_thought":"the anchor is on line 3","tool":"read","path":"a.rs"}"#)
                .unwrap();
        assert_eq!(
            action,
            SchemaAction::Call {
                name: "read".to_string(),
                arguments: json!({"path": "a.rs"}),
            },
            "the reasoning is for the model; forwarding it would fail argument \
             validation on every tool Legion offers"
        );

        let unprefixed =
            parse_action(r#"{"thought":"stray","tool":"read","path":"a.rs"}"#).unwrap();
        assert_eq!(
            unprefixed,
            SchemaAction::Call {
                name: "read".to_string(),
                arguments: json!({"path": "a.rs"}),
            },
            "a model ignoring its grammar should cost a field, not the call"
        );
    }

    #[test]
    fn a_call_parses_with_the_discriminator_removed() {
        let action = parse_action(r#"{"tool":"read","path":"a.rs"}"#).unwrap();
        assert_eq!(
            action,
            SchemaAction::Call {
                name: "read".to_string(),
                arguments: json!({"path": "a.rs"}),
            },
            "`tool` is the discriminator, not an argument the executor takes"
        );
    }

    #[test]
    fn a_done_response_parses_as_done() {
        let action = parse_action(r#"{"tool":"done","summary":"all set"}"#).unwrap();
        assert_eq!(
            action,
            SchemaAction::Done {
                summary: "all set".to_string()
            }
        );
    }

    #[test]
    fn surrounding_noise_is_tolerated() {
        let action = parse_action("Sure!\n```json\n{\"tool\":\"read\",\"path\":\"a.rs\"}\n```")
            .expect("some endpoints add a fence even under a grammar");
        assert!(matches!(action, SchemaAction::Call { .. }));
    }

    #[test]
    fn a_brace_inside_a_value_does_not_end_the_object() {
        let action =
            parse_action(r#"prefix {"tool":"edit-as-proposal","new_str":"fn f() { }"} suffix"#)
                .unwrap();
        let SchemaAction::Call { arguments, .. } = action else {
            panic!("expected a call");
        };
        assert_eq!(arguments["new_str"], "fn f() { }");
    }

    #[test]
    fn a_response_without_a_tool_is_not_an_action() {
        assert!(
            parse_action(r#"{"path":"a.rs"}"#).is_none(),
            "inferring the tool would fabricate a call the model did not make"
        );
        assert!(parse_action("not json at all").is_none());
        assert!(parse_action(r#"{"tool":42}"#).is_none());
    }
}
