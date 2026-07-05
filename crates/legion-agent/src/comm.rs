//! Parsed communication log lines for Legion agent workflows.

/// Stable communication tags used by agent log lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCommTag {
    /// Planning/update tag.
    Plan,
    /// Implementation/update tag.
    Write,
    /// Test/verification tag.
    Test,
    /// Review/approval tag.
    Review,
    /// Error/failure tag.
    Error,
    /// Approval gate tag.
    Approval,
    /// Completion tag.
    Complete,
}

impl AgentCommTag {
    /// Stable tag set exposed by the parser.
    pub const ALL: [Self; 7] = [
        Self::Plan,
        Self::Write,
        Self::Test,
        Self::Review,
        Self::Error,
        Self::Approval,
        Self::Complete,
    ];
}

/// Parsed `[timestamp] [TAG] actor: message` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAgentCommLine {
    /// Timestamp string from the bracketed prefix.
    pub timestamp: String,
    /// Parsed stable tag.
    pub tag: AgentCommTag,
    /// Actor label before the first colon.
    pub actor: String,
    /// Message text after the first colon.
    pub message: String,
}

/// Parses the documented tagged agent-communication format.
///
/// Returns `None` for freeform text or malformed tagged lines.
pub fn parse_agent_comm_line(input: &str) -> Option<ParsedAgentCommLine> {
    let rest = input.strip_prefix('[')?;
    let (timestamp, rest) = rest.split_once("] [")?;
    let (tag_raw, rest) = rest.split_once("] ")?;
    let (actor, message) = rest.split_once(": ")?;
    if timestamp.is_empty() || actor.is_empty() || message.is_empty() {
        return None;
    }
    let tag = match tag_raw {
        "PLAN" => AgentCommTag::Plan,
        "WRITE" => AgentCommTag::Write,
        "TEST" => AgentCommTag::Test,
        "REVIEW" => AgentCommTag::Review,
        "ERROR" => AgentCommTag::Error,
        "APPROVAL" => AgentCommTag::Approval,
        "COMPLETE" => AgentCommTag::Complete,
        _ => return None,
    };
    Some(ParsedAgentCommLine {
        timestamp: timestamp.to_string(),
        tag,
        actor: actor.to_string(),
        message: message.to_string(),
    })
}
