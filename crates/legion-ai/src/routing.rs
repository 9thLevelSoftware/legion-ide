//! Which tools a turn is offered, decided from what the turn is asking for.
//!
//! Ported from SmallCode's `two_stage_router.js` and `action_classifier.js`
//! per ADR-0049. Semantics and test vectors are reused; nothing here executes
//! anything or relaxes a boundary — the scope still decides what is *allowed*,
//! and this only narrows what is *advertised* within it.
//!
//! The narrowing matters more than it sounds. `tool_defs_from_registry` already
//! records why: under a constrained-decoding transport every advertised tool is
//! an equally legal branch of the grammar, and a benchmark run that advertised
//! `terminal-command` outside the scope blocked on all 13 tasks because the
//! model kept choosing a branch that could only fail. A tool that cannot help
//! with the question being asked is the same trap one step earlier.

use legion_protocol::tools::LegionToolKind;

/// What a turn is asking the agent to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionClass {
    /// Read, search, explain — nothing about the workspace should change.
    Query,
    /// Change something, or run something that can.
    Mutate,
}

/// Words that only appear when a change is wanted.
///
/// Matched on whole words against the lowercased directive. Substring matching
/// was rejected deliberately: "increment" contains "increment" but "rewrite"
/// also contains "write", and "documentation" contains "document" — a
/// substring rule reads intent into words that carry none.
const MUTATE_SIGNALS: &[&str] = &[
    "add",
    "append",
    "apply",
    "build",
    "change",
    "convert",
    "create",
    "delete",
    "edit",
    "extract",
    "fix",
    "format",
    "generate",
    "implement",
    "insert",
    "install",
    "migrate",
    "modify",
    "move",
    "patch",
    "refactor",
    "remove",
    "rename",
    "replace",
    "rewrite",
    "run",
    "set",
    "split",
    "test",
    "update",
    "upgrade",
    "write",
];

/// Words that mark a directive as asking rather than instructing.
///
/// `Query` requires one of these. Absence of a mutation word is not evidence
/// of a question: "Do the task." carries neither, and reading it as a query
/// would withhold the edit tool from a directive whose whole content is an
/// instruction to act.
const QUERY_SIGNALS: &[&str] = &[
    "analyse",
    "analyze",
    "compare",
    "describe",
    "diagnose",
    "explain",
    "find",
    "how",
    "identify",
    "inspect",
    "investigate",
    "list",
    "locate",
    "look",
    "read",
    "review",
    "search",
    "show",
    "summarise",
    "summarize",
    "trace",
    "what",
    "when",
    "where",
    "which",
    "who",
    "why",
];

/// Words that join a second clause, which usually carries the real work.
///
/// "Investigate and resolve the crash" opens with a question word and is an
/// instruction. A verb list can never be complete -- `resolve`, `address`,
/// `sort out`, `deal with` are all missing and always will be -- so a second
/// clause is treated as evidence of work rather than something to be
/// recognised word by word.
const CLAUSE_JOINERS: &[&str] = &[" and ", " then ", ", and ", "; ", " & "];

/// Classify a directive as a query or a mutation.
///
/// Biased toward `Mutate`, and the asymmetry is the whole design. Offering a
/// tool that goes unused costs tokens; withholding one the task needs makes the
/// task impossible and the model cannot say so — it can only fail in a way that
/// looks like incapacity.
///
/// So `Query` has to be earned and everything else is `Mutate`. A mutation
/// signal wins outright. Otherwise the directive must *open* as a question and
/// carry no second clause; anything else — an unrecognised instruction, a
/// question with work attached — is `Mutate`.
///
/// Two earlier versions got this wrong in the same direction and are worth
/// recording. The first defaulted to `Query` when no mutation verb appeared, so
/// "Do the task." was handed a tool set with no way to do it. The second
/// accepted a query word anywhere, so "Investigate and resolve the crash" lost
/// its edit tool because `resolve` is in no verb list — and no verb list will
/// ever hold all of `resolve`, `address`, `sort out`, `deal with`. Both were the
/// exact failure this asymmetry exists to prevent, arriving through the code
/// meant to prevent it.
pub fn classify_action(directive: &str) -> ActionClass {
    let lowered = directive.to_lowercase();
    // An underscore is part of a word, not a separator.
    //
    // Splitting on it shreds identifiers into their parts, and the parts carry
    // meanings the identifier does not: "show me every caller of
    // `resolve_edit_span`" becomes ... `edit` ... and is read as an instruction
    // to change something. Directives about this codebase are full of
    // snake_case, so that is the common case rather than a corner one. Hyphens
    // stay separators, because in English prose they join words that are still
    // words.
    let words: Vec<&str> = lowered
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|word| !word.is_empty())
        .collect();
    if words.iter().any(|word| MUTATE_SIGNALS.contains(word)) {
        return ActionClass::Mutate;
    }
    // A question, and only a question.
    //
    // `Query` used to need a question word anywhere in the directive, which
    // read "Investigate and resolve the crash" as one: `investigate` is a query
    // signal and `resolve` is not in any verb list -- and never reliably will
    // be, because that list cannot be completed. The edit tool was then withheld
    // from an explicit repair request, which is the failure the whole asymmetry
    // exists to prevent, arriving through the half of the rule meant to prevent
    // it.
    //
    // So `Query` now needs the directive to *open* as a question and to carry
    // no second clause. Both conditions are cheap to satisfy honestly and hard
    // to satisfy by accident, and everything else falls to `Mutate`, where an
    // unused tool costs tokens instead of the task.
    let opens_as_a_question = words
        .first()
        .is_some_and(|word| QUERY_SIGNALS.contains(word))
        || lowered.trim_end().ends_with('?');
    let carries_a_second_clause = CLAUSE_JOINERS.iter().any(|joiner| lowered.contains(joiner));
    if opens_as_a_question && !carries_a_second_clause {
        return ActionClass::Query;
    }
    ActionClass::Mutate
}

/// Narrow an allowed tool set to the ones a turn of this class can use.
///
/// Only `EditAsProposal` is withheld, and only from a query. That is a smaller
/// cut than SmallCode makes, for a reason worth stating: `TerminalCommand` looks
/// like the dangerous one and is not the right thing to gate here, because "run
/// the tests and tell me what fails" is a read-intent directive that cannot be
/// answered without it. Gating the terminal on this classification would break
/// that ask while doing nothing about the actual risk, which the capability
/// broker and sandbox own.
///
/// Withholding the edit tool from a directive that asked a question is the part
/// that is unambiguously right: there is no reading of "where is this defined"
/// that wants a workspace edit, and a small model offered one will eventually
/// take it.
pub fn tools_for_action(class: ActionClass, allowed: &[LegionToolKind]) -> Vec<LegionToolKind> {
    allowed
        .iter()
        .copied()
        .filter(|tool| {
            !matches!(
                (class, tool),
                (ActionClass::Query, LegionToolKind::EditAsProposal)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_a_query() {
        for directive in [
            "where is the proposal ledger defined",
            "explain how the capability broker decides",
            "show me every caller of resolve_edit_span",
            "which crate owns the sandbox",
        ] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Query,
                "{directive:?} asks for nothing to change"
            );
        }
    }

    #[test]
    fn an_instruction_to_change_something_is_a_mutation() {
        for directive in [
            "rename the run_id field to correlation_id",
            "fix the off-by-one in the viewport",
            "add a test for the ambiguous anchor case",
            "run the test suite",
        ] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Mutate,
                "{directive:?} asks for a change or an execution"
            );
        }
    }

    /// A mixed directive is a mutation.
    ///
    /// "Explain X and then fix it" carries both signals, and the expensive
    /// mistake is withholding the edit tool from a turn that needed it.
    #[test]
    fn a_directive_carrying_both_signals_is_a_mutation() {
        assert_eq!(
            classify_action("explain why this panics and then fix it"),
            ActionClass::Mutate
        );
    }

    /// An identifier is one word, so its parts do not vote.
    ///
    /// Directives about this codebase are full of snake_case, and splitting on
    /// underscores turns `resolve_edit_span` into a request to edit something.
    #[test]
    fn an_identifier_is_not_read_as_an_instruction() {
        assert_eq!(
            classify_action("show me every caller of resolve_edit_span"),
            ActionClass::Query
        );
        assert_eq!(
            classify_action("what does apply_edit_from_arguments return"),
            ActionClass::Query
        );
    }

    /// Signals are whole words, not substrings.
    ///
    /// A substring rule sees "write" inside "rewrite" and "set" inside
    /// "offset", so a question about an offset would be classified as a
    /// mutation and the narrowing would never fire.
    #[test]
    fn a_signal_buried_inside_another_word_does_not_count() {
        assert_eq!(
            classify_action("what is the byte offset of the viewport cursor"),
            ActionClass::Query,
            "`offset` contains `set` and means nothing of the kind"
        );
        assert_eq!(
            classify_action("describe the documentation layout"),
            ActionClass::Query,
            "`documentation` is not an instruction to document"
        );
    }

    /// A directive with no signal either way keeps every tool.
    ///
    /// "Do the task." is neither a recognised question nor a recognised
    /// instruction, and the safe reading is the one that can still act. An
    /// earlier version defaulted to `Query` here and withheld the edit tool
    /// from a directive whose entire content was an instruction to act; the
    /// scripted cross-check loop caught it.
    #[test]
    fn a_directive_with_no_signal_is_treated_as_a_mutation() {
        for directive in ["Do the task.", "proceed", "continue from the plan", ""] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Mutate,
                "{directive:?} carries no question, so it must keep the full tool set"
            );
        }
    }

    /// A question with work attached keeps its tools.
    ///
    /// "Investigate and resolve the crash" opens as a question and is an
    /// instruction. `resolve` is in no verb list and no verb list will ever
    /// hold every synonym for it, so the second clause is the evidence.
    #[test]
    fn a_question_with_work_attached_is_a_mutation() {
        for directive in [
            "Investigate and resolve the crash",
            "explain the failure and address it",
            "look at the parser, and fix whatever is wrong",
            "review the module then sort out the naming",
        ] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Mutate,
                "{directive:?} asks for work in its second clause"
            );
        }
    }

    /// A question word buried mid-sentence does not make a directive a query.
    #[test]
    fn a_query_word_that_does_not_open_the_directive_is_not_a_question() {
        assert_eq!(
            classify_action("make the error message explain what went wrong"),
            ActionClass::Mutate
        );
    }

    /// A trailing question mark is enough on its own.
    #[test]
    fn a_question_mark_marks_a_question() {
        assert_eq!(
            classify_action("is the ledger projection rebuilt per frame?"),
            ActionClass::Query
        );
    }

    #[test]
    fn a_query_is_not_offered_the_edit_tool() {
        let allowed = [
            LegionToolKind::Read,
            LegionToolKind::Grep,
            LegionToolKind::EditAsProposal,
            LegionToolKind::TerminalCommand,
        ];
        let offered = tools_for_action(ActionClass::Query, &allowed);

        assert!(!offered.contains(&LegionToolKind::EditAsProposal));
        assert!(
            offered.contains(&LegionToolKind::Read) && offered.contains(&LegionToolKind::Grep),
            "reading tools must survive: they are what answers the question"
        );
    }

    /// The terminal survives a query, deliberately.
    ///
    /// "Run the tests and tell me what fails" reads as a question and cannot be
    /// answered without it. Gating execution on a keyword classification would
    /// break that while doing nothing about the real risk, which the capability
    /// broker and the sandbox own.
    #[test]
    fn a_query_keeps_the_terminal() {
        let allowed = [LegionToolKind::Read, LegionToolKind::TerminalCommand];
        let offered = tools_for_action(ActionClass::Query, &allowed);

        assert!(offered.contains(&LegionToolKind::TerminalCommand));
    }

    #[test]
    fn a_mutation_is_offered_everything_the_scope_allows() {
        let allowed = [
            LegionToolKind::Read,
            LegionToolKind::EditAsProposal,
            LegionToolKind::TerminalCommand,
        ];
        let offered = tools_for_action(ActionClass::Mutate, &allowed);

        assert_eq!(offered, allowed.to_vec(), "narrowing is for queries only");
    }

    /// Narrowing never widens.
    ///
    /// The scope decides what is allowed; this only ever removes from that set.
    /// A tool appearing here that the scope withheld would be an escalation
    /// dressed as a token optimisation.
    #[test]
    fn narrowing_never_introduces_a_tool_the_scope_withheld() {
        let allowed = [LegionToolKind::Read];
        for class in [ActionClass::Query, ActionClass::Mutate] {
            for tool in tools_for_action(class, &allowed) {
                assert!(
                    allowed.contains(&tool),
                    "{tool:?} was not allowed by the scope"
                );
            }
        }
    }
}
