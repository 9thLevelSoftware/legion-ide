//! Deterministic Phase 8 local terminal fixture runtime.

#![warn(missing_docs)]

use devil_platform::{PtyRequest, PtyService};
use devil_protocol::{
    RedactionHint, TerminalAuditRecord, TerminalLaunchPolicyContract, TerminalOutputChunk,
    TerminalRuntimeState, WorkspaceTrustState, validate_terminal_audit_record,
};
use thiserror::Error;

/// Terminal fixture error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TerminalFixtureError {
    /// Fixture runtime is disabled.
    #[error("terminal fixture is disabled")]
    Disabled,
    /// Launch policy rejected the request.
    #[error("terminal launch denied: {reason}")]
    Denied {
        /// Denial reason.
        reason: String,
    },
    /// Output metadata exceeded configured bounds.
    #[error("terminal output exceeded bounds: {reason}")]
    LimitExceeded {
        /// Limit reason.
        reason: String,
    },
}

/// Terminal runtime error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TerminalRuntimeError {
    /// Runtime is disabled.
    #[error("terminal runtime is disabled")]
    Disabled,
    /// Launch was denied by policy or metadata validation.
    #[error("terminal runtime denied: {reason}")]
    Denied {
        /// Denial reason.
        reason: String,
    },
    /// Backend failed.
    #[error("terminal backend failed: {reason}")]
    Backend {
        /// Backend failure reason.
        reason: String,
    },
}

/// Production terminal runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeConfig {
    /// Whether terminal runtime is enabled.
    pub enabled: bool,
    /// Maximum projected output bytes.
    pub max_output_bytes: u64,
}

impl TerminalRuntimeConfig {
    /// Return an enabled terminal runtime configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TerminalRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Terminal launch request for the process-backed degraded runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeLaunchRequest {
    /// Launch policy contract.
    pub policy: TerminalLaunchPolicyContract,
    /// Command executable selected by policy-owned caller.
    pub command: String,
    /// Command arguments selected by policy-owned caller.
    pub args: Vec<String>,
}

/// Terminal launch outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeLaunchOutcome {
    /// Metadata-only audit record.
    pub audit: TerminalAuditRecord,
    /// Bounded redacted projection chunk.
    pub output: TerminalOutputChunk,
}

/// Terminal fixture configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFixtureConfig {
    /// Whether deterministic terminal fixture behavior is enabled.
    pub enabled: bool,
    /// Maximum output bytes accepted by the fixture.
    pub max_output_bytes: u64,
}

impl TerminalFixtureConfig {
    /// Return an enabled deterministic fixture configuration.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }
}

impl Default for TerminalFixtureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_output_bytes: 256 * 1024,
        }
    }
}

/// Deterministic terminal fixture runtime.
#[derive(Debug, Clone)]
pub struct TerminalFixtureRuntime {
    config: TerminalFixtureConfig,
}

impl TerminalFixtureRuntime {
    /// Construct a runtime from configuration.
    pub fn new(config: TerminalFixtureConfig) -> Self {
        Self { config }
    }

    /// Launch a metadata-only deterministic fixture session.
    pub fn launch(
        &self,
        policy: TerminalLaunchPolicyContract,
    ) -> Result<TerminalAuditRecord, TerminalFixtureError> {
        if !self.config.enabled {
            return Err(TerminalFixtureError::Disabled);
        }
        if policy.schema_version == 0
            || policy.principal_id.0.trim().is_empty()
            || policy.workspace_id.0 == 0
            || policy.trust_state != WorkspaceTrustState::Trusted
            || policy.capability_id.0.trim().is_empty()
            || policy.output_byte_limit == 0
            || policy.output_byte_limit > self.config.max_output_bytes
            || policy.timeout_seconds == 0
        {
            return Err(TerminalFixtureError::Denied {
                reason: "terminal launch policy is incomplete or outside bounds".to_string(),
            });
        }
        let record = TerminalAuditRecord {
            session_id: devil_protocol::TerminalSessionId(policy.workspace_id.0 as u64),
            state: TerminalRuntimeState::Running,
            event_sequence: devil_protocol::EventSequence(policy.workspace_id.0 as u64),
            correlation_id: devil_protocol::CorrelationId(policy.workspace_id.0 as u64),
            causality_id: devil_protocol::CausalityId(uuid_from_value(
                policy.workspace_id.0 as u64,
            )),
            metadata_summary: format!(
                "state=running cwd_policy={} output_limit={} timeout_seconds={}",
                policy.cwd_policy, policy.output_byte_limit, policy.timeout_seconds
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_terminal_audit_record(&record).map_err(|err| TerminalFixtureError::Denied {
            reason: err.message,
        })?;
        Ok(record)
    }

    /// Build a bounded redacted output chunk for projection-only consumers.
    pub fn output_chunk(
        &self,
        record: &TerminalAuditRecord,
        redacted_payload: impl Into<String>,
        byte_count: u64,
    ) -> Result<TerminalOutputChunk, TerminalFixtureError> {
        if !self.config.enabled {
            return Err(TerminalFixtureError::Disabled);
        }
        let redacted_payload = redacted_payload.into();
        if byte_count > self.config.max_output_bytes {
            return Err(TerminalFixtureError::LimitExceeded {
                reason: "terminal output exceeds configured byte limit".to_string(),
            });
        }
        Ok(TerminalOutputChunk {
            session_id: record.session_id,
            sequence: record.event_sequence,
            redacted_payload,
            byte_count,
            truncated: byte_count == self.config.max_output_bytes,
            redaction: RedactionHint::MetadataOnly,
            schema_version: 1,
        })
    }

    /// Produce a deterministic exit audit record.
    pub fn exit(&self, record: &TerminalAuditRecord) -> TerminalAuditRecord {
        TerminalAuditRecord {
            state: TerminalRuntimeState::Exited,
            metadata_summary: "state=exited exit_code=0".to_string(),
            ..record.clone()
        }
    }
}

/// Process-backed terminal runtime using the platform PTY boundary.
pub struct TerminalRuntime<P> {
    config: TerminalRuntimeConfig,
    pty: P,
}

impl<P: PtyService> TerminalRuntime<P> {
    /// Construct a terminal runtime from configuration and platform PTY service.
    pub fn new(config: TerminalRuntimeConfig, pty: P) -> Self {
        Self { config, pty }
    }

    /// Launch a terminal command and return metadata-only audit/projection records.
    pub fn launch(
        &self,
        request: TerminalRuntimeLaunchRequest,
    ) -> Result<TerminalRuntimeLaunchOutcome, TerminalRuntimeError> {
        if !self.config.enabled {
            return Err(TerminalRuntimeError::Disabled);
        }
        validate_launch_policy(&request.policy, self.config.max_output_bytes)?;
        if request.command.trim().is_empty() {
            return Err(TerminalRuntimeError::Denied {
                reason: "terminal command metadata is required".to_string(),
            });
        }
        let session = self
            .pty
            .spawn_pty(&PtyRequest {
                command: request.command,
                args: request.args,
                cwd: None,
            })
            .map_err(|err| TerminalRuntimeError::Backend {
                reason: err.to_string(),
            })?;
        let redacted = redact_terminal_projection(&session.output, self.config.max_output_bytes);
        let byte_count = session
            .output
            .len()
            .min(self.config.max_output_bytes as usize) as u64;
        let audit = TerminalAuditRecord {
            session_id: devil_protocol::TerminalSessionId(request.policy.workspace_id.0 as u64),
            state: TerminalRuntimeState::Degraded,
            event_sequence: devil_protocol::EventSequence(request.policy.workspace_id.0 as u64),
            correlation_id: devil_protocol::CorrelationId(request.policy.workspace_id.0 as u64),
            causality_id: devil_protocol::CausalityId(uuid_from_value(
                request.policy.workspace_id.0 as u64,
            )),
            metadata_summary: format!(
                "state=degraded backend=process_lifecycle pty=false output_bytes={} truncated={}",
                byte_count,
                session.output.len() as u64 > self.config.max_output_bytes
            ),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        };
        validate_terminal_audit_record(&audit).map_err(|err| TerminalRuntimeError::Denied {
            reason: err.message,
        })?;
        Ok(TerminalRuntimeLaunchOutcome {
            output: TerminalOutputChunk {
                session_id: audit.session_id,
                sequence: audit.event_sequence,
                redacted_payload: redacted,
                byte_count,
                truncated: session.output.len() as u64 > self.config.max_output_bytes,
                redaction: RedactionHint::MetadataOnly,
                schema_version: 1,
            },
            audit,
        })
    }
}

fn validate_launch_policy(
    policy: &TerminalLaunchPolicyContract,
    max_output_bytes: u64,
) -> Result<(), TerminalRuntimeError> {
    if policy.schema_version == 0
        || policy.principal_id.0.trim().is_empty()
        || policy.workspace_id.0 == 0
        || policy.trust_state != WorkspaceTrustState::Trusted
        || policy.capability_id.0 != "terminal.launch"
        || policy.output_byte_limit == 0
        || policy.output_byte_limit > max_output_bytes
        || policy.timeout_seconds == 0
    {
        return Err(TerminalRuntimeError::Denied {
            reason: "terminal launch policy is incomplete or outside bounds".to_string(),
        });
    }
    Ok(())
}

fn redact_terminal_projection(output: &str, limit: u64) -> String {
    let mut projected = output.replace("secret", "[redacted]");
    if projected.len() > limit as usize {
        projected.truncate(limit as usize);
    }
    projected
}

fn uuid_from_value(value: u64) -> uuid::Uuid {
    uuid::Uuid::from_u128(0x018f_0000_0000_7000_8000_1000_0000_0000_u128 + value as u128)
}

#[cfg(test)]
mod tests {
    use devil_platform::{PlatformError, PtySession};
    use devil_protocol::{CapabilityId, PrincipalId, WorkspaceId};

    use super::*;

    #[derive(Debug, Clone)]
    struct FakePty {
        output: String,
    }

    impl PtyService for FakePty {
        fn spawn_pty(&self, request: &PtyRequest) -> Result<PtySession, PlatformError> {
            if request.command == "fail" {
                return Err(PlatformError::PtyUnavailable {
                    reason: "fake failure".to_string(),
                });
            }
            Ok(PtySession {
                id: "fake".to_string(),
                output: self.output.clone(),
            })
        }
    }

    fn policy() -> TerminalLaunchPolicyContract {
        TerminalLaunchPolicyContract {
            principal_id: PrincipalId("tester".to_string()),
            workspace_id: WorkspaceId(42),
            trust_state: WorkspaceTrustState::Trusted,
            capability_id: CapabilityId("terminal.launch".to_string()),
            cwd_policy: "workspace-contained".to_string(),
            output_byte_limit: 1024,
            timeout_seconds: 60,
            schema_version: 1,
        }
    }

    #[test]
    fn terminal_fixture_is_default_off() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::default());
        assert!(matches!(
            runtime.launch(policy()),
            Err(TerminalFixtureError::Disabled)
        ));
    }

    #[test]
    fn terminal_fixture_launches_metadata_only_and_exits() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::enabled());
        let record = runtime.launch(policy()).expect("launch");
        assert_eq!(record.state, TerminalRuntimeState::Running);
        assert!(!record.metadata_summary.contains("terminal_output"));

        let chunk = runtime
            .output_chunk(&record, "redacted output", 15)
            .expect("chunk");
        assert_eq!(chunk.redaction, RedactionHint::MetadataOnly);
        assert!(!chunk.redacted_payload.contains("secret"));

        let exit = runtime.exit(&record);
        assert_eq!(exit.state, TerminalRuntimeState::Exited);
    }

    #[test]
    fn terminal_fixture_rejects_untrusted_launch() {
        let runtime = TerminalFixtureRuntime::new(TerminalFixtureConfig::enabled());
        let denied = TerminalLaunchPolicyContract {
            trust_state: WorkspaceTrustState::Untrusted,
            ..policy()
        };
        assert!(matches!(
            runtime.launch(denied),
            Err(TerminalFixtureError::Denied { .. })
        ));
    }

    #[test]
    fn terminal_runtime_default_off_denies_launch() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::default(),
            FakePty {
                output: String::new(),
            },
        );
        assert!(matches!(
            runtime.launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            }),
            Err(TerminalRuntimeError::Disabled)
        ));
    }

    #[test]
    fn terminal_runtime_launch_creates_metadata_only_degraded_audit() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty {
                output: "hello secret".to_string(),
            },
        );
        let outcome = runtime
            .launch(TerminalRuntimeLaunchRequest {
                policy: policy(),
                command: "test".to_string(),
                args: vec![],
            })
            .expect("launch");
        assert_eq!(outcome.audit.state, TerminalRuntimeState::Degraded);
        assert!(outcome.audit.metadata_summary.contains("pty=false"));
        assert!(!outcome.audit.metadata_summary.contains("terminal_output"));
        assert!(outcome.output.redacted_payload.contains("[redacted]"));
    }

    #[test]
    fn terminal_runtime_rejects_untrusted_policy() {
        let runtime = TerminalRuntime::new(
            TerminalRuntimeConfig::enabled(),
            FakePty {
                output: String::new(),
            },
        );
        assert!(matches!(
            runtime.launch(TerminalRuntimeLaunchRequest {
                policy: TerminalLaunchPolicyContract {
                    trust_state: WorkspaceTrustState::Untrusted,
                    ..policy()
                },
                command: "test".to_string(),
                args: vec![],
            }),
            Err(TerminalRuntimeError::Denied { .. })
        ));
    }
}
