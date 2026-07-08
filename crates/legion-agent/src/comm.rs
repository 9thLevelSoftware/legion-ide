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

    /// Stable uppercase tag label used in persisted communication rows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "PLAN",
            Self::Write => "WRITE",
            Self::Test => "TEST",
            Self::Review => "REVIEW",
            Self::Error => "ERROR",
            Self::Approval => "APPROVAL",
            Self::Complete => "COMPLETE",
        }
    }
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

/// Formats a documented metadata-only communication row.
pub fn format_agent_comm_line(
    timestamp: impl AsRef<str>,
    tag: AgentCommTag,
    actor: impl AsRef<str>,
    message: impl AsRef<str>,
) -> String {
    format!(
        "[{}] [{}] {}: {}",
        timestamp.as_ref(),
        tag.label(),
        actor.as_ref(),
        message.as_ref()
    )
}

#[cfg(test)]
mod tests {
    use super::{AgentCommTag, format_agent_comm_line, parse_agent_comm_line};

    #[test]
    fn documented_comm_line_round_trips_all_tags() {
        for tag in AgentCommTag::ALL {
            let line = format_agent_comm_line(
                "2026-07-08T12:00:00Z",
                tag,
                "worker:console",
                "metadata-only event",
            );
            let parsed = parse_agent_comm_line(&line).expect("formatted line must parse");

            assert_eq!(parsed.timestamp, "2026-07-08T12:00:00Z");
            assert_eq!(parsed.tag, tag);
            assert_eq!(parsed.actor, "worker:console");
            assert_eq!(parsed.message, "metadata-only event");
        }
    }
}
