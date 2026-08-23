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
const CLAUSE_JOINERS: &[&str] = &[" and ", " then ", " & ", " - "];

/// Punctuation that ends one instruction, and so may begin another.
///
/// "Investigate the crash. Resolve it." and "Investigate the crash, resolve
/// it." both carry their real work in a clause no conjunction check can see:
/// the first has no joiner at all, the second has a bare comma. Only the
/// opening clause was ever classified, and `resolve` is in no verb list -- so
/// both lost the edit tool for a repair the user asked for in as many words.
///
/// Semicolons, commas, colons and dashes are here rather than in
/// [`CLAUSE_JOINERS`] because a separator does not need a word after it to
/// separate. This list has now been extended four times, once per punctuation
/// mark somebody thought of -- period, comma, colon, dash -- which is itself
/// the argument for enumerating separators instead of verbs: the punctuation is
/// finite, and the set of English words meaning "fix it" is not.
///
/// The plain hyphen is **not** here, and cannot be: it joins words that are
/// still words, so `re-run the tests` would read as two clauses on the strength
/// of its spelling. A hyphen used as a dash is written with spaces around it,
/// which is why `" - "` is a joiner instead.
///
/// The cost is that "what does this do, exactly?" and "where is `Foo::bar`
/// defined?" both read as two clauses and keep an edit tool they will not use.
/// That is the same trade the rest of this module makes, and it falls on the
/// side measured in tokens.
const CLAUSE_ENDS: &[char] = &['.', '!', '?', '\n', ';', ',', ':', '\u{2013}', '\u{2014}'];

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
/// Three earlier versions got this wrong in the same direction, and the list is
/// the argument for how conservative the rule now is. The first defaulted to
/// `Query` when no mutation verb appeared, so "Do the task." was handed a tool
/// set with no way to do it. The second accepted a query word anywhere, so
/// "Investigate and resolve the crash" lost its edit tool because `resolve` is
/// in no verb list — and none will ever hold all of `resolve`, `address`, `sort
/// out`, `deal with`. The third accepted a trailing question mark as evidence,
/// so "Can you resolve this crash?" lost it to politeness.
///
/// The reverse errors are real and deliberately tolerated. "Which command
/// should I run?", "Compare the parser and lexer" and "What does this do,
/// exactly?" are all questions and all classify as `Mutate` — the first because
/// `run` is a mutation signal, the second because a conjunction is read as a
/// second clause, the third because a comma is. Each keeps an edit tool a
/// question will not use, which costs tokens and one grammar branch. That is
/// the whole trade: this side of the line is measured in tokens, the other side
/// in tasks that cannot be completed, so the rule stays on this side of it.
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
    // A question mark is not enough, and that was the hole.
    //
    // "Can you resolve this crash?" ends in one, opens with a word that is not
    // interrogative, and contains no recognised mutation verb -- so the mark
    // alone made it a query and the edit tool was withheld from a plain repair
    // request. Politeness is not a signal about what is being asked for.
    let opens_as_a_question = words
        .first()
        .is_some_and(|word| QUERY_SIGNALS.contains(word));
    let carries_a_second_clause = CLAUSE_JOINERS.iter().any(|joiner| lowered.contains(joiner))
        || text_follows_a_clause_end(&lowered);
    if opens_as_a_question && !carries_a_second_clause {
        return ActionClass::Query;
    }
    ActionClass::Mutate
}

/// Whether anything follows the first clause.
///
/// Trailing punctuation on a single clause does not count -- "where is this
/// defined?" is one question with a mark on the end. What counts is text *after*
/// a clause ends, because that text is a second instruction the classification
/// of the first says nothing about.
fn text_follows_a_clause_end(lowered: &str) -> bool {
    lowered
        .split_once(CLAUSE_ENDS)
        .is_some_and(|(_, rest)| !rest.trim().is_empty())
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

    /// A polite question asking for work is work.
    ///
    /// "Can you resolve this crash?" is a repair request wearing a question
    /// mark. `resolve` is in no verb list and never will be, so the mark was
    /// the only evidence and it was the wrong evidence -- politeness says
    /// nothing about what is being asked for.
    #[test]
    fn a_polite_request_for_work_is_a_mutation() {
        for directive in [
            "Can you resolve this crash?",
            "Could you take a look at the parser and sort it out?",
            "would you mind addressing the failing test?",
        ] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Mutate,
                "{directive:?} is asking for work"
            );
        }
    }

    /// A second clause is a second instruction, however it is punctuated.
    ///
    /// "Investigate the crash. Resolve it." and "Investigate the crash, resolve
    /// it." both carry their work past a separator no conjunction check saw --
    /// a period joins nothing, and a bare comma was not a joiner. Only the
    /// opening clause was classified, so a directive doing two things had one
    /// of them unexamined.
    #[test]
    fn a_second_clause_is_treated_as_work() {
        for directive in [
            "Investigate the crash. Resolve it.",
            "Investigate the crash, resolve it.",
            "Investigate the crash: resolve it.",
            "Investigate the crash \u{2014} resolve it.",
            "Investigate the crash - resolve it.",
            "Explain the failure.\nThen make it stop.",
            "What does this function do? Rewrite it to be clearer.",
            "Review the parser; make it stop panicking.",
        ] {
            assert_eq!(
                classify_action(directive),
                ActionClass::Mutate,
                "{directive:?} carries a second instruction"
            );
        }
    }

    /// A hyphen inside a word is not a dash between clauses.
    ///
    /// `well-known` is one word however it is spelled, and reading its hyphen
    /// as a clause separator would make every hyphenated phrase a second
    /// instruction. (`re-run` would also be one word, but `run` is a mutation
    /// signal and the word splitter treats hyphens as boundaries, so that one
    /// is a mutation for a different and deliberate reason.)
    #[test]
    fn a_hyphenated_word_is_not_two_clauses() {
        assert_eq!(
            classify_action("where is the well-known helper defined"),
            ActionClass::Query
        );
    }

    /// One clause with a mark on the end is still one clause.
    #[test]
    fn trailing_punctuation_alone_is_not_a_second_clause() {
        assert_eq!(
            classify_action("where is the proposal ledger defined?"),
            ActionClass::Query
        );
    }

    /// Some plain questions keep the edit tool, and that is the accepted cost.
    ///
    /// "Which command should I run?" contains a mutation signal and
    /// "Compare the parser and lexer" contains a conjunction, so both are
    /// `Mutate`. Each keeps a tool a question will not use. Pinned rather than
    /// hidden: this is the side of the trade the design chose, and a future
    /// change that makes either a `Query` should have to notice it is moving
    /// the line toward the failure mode that breaks tasks.
    #[test]
    fn some_questions_are_over_served_and_that_is_the_trade() {
        assert_eq!(
            classify_action("Which command should I run?"),
            ActionClass::Mutate
        );
        assert_eq!(
            classify_action("Compare the parser and lexer"),
            ActionClass::Mutate
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
