//! Debug workflow: DAP session lifecycle, breakpoints, and the live adapter path.
//!
//! Moved verbatim out of `lib.rs` (P2.F3.T2). `lib.rs` is the workspace merge
//! chokepoint, so debug work lands here instead of growing it further. The
//! glob import keeps the moved code resolving against the same in-crate items
//! it did before the move; nothing else changed in this commit.

use super::*;
use legion_security::DEBUG_ADAPTER_LAUNCH_CAPABILITY;

/// File stem of the in-tree CI fake adapter (`legion-debug`'s `fake_dap_adapter`
/// bin). Only test seams add it to the adapter allowlist.
const FAKE_DAP_ADAPTER_BINARY: &str = "fake_dap_adapter";

/// Owned live DAP adapter process (B5); not Clone — process handles.
pub(crate) struct LiveDebugSession {
    pub(crate) session: LiveDapSession,
    pub(crate) thread_id: u64,
    pub(crate) session_id: DebugSessionId,
    pub(crate) adapter_type: String,
    pub(crate) is_fake: bool,
    /// C4: platform sandbox lifetime (e.g. Windows job) for sandboxed adapter.
    pub(crate) _sandbox_guard: Option<legion_sandbox::spawn_stdio::PlatformGuard>,
}

/// Background wait for next stop after non-blocking continue (B7).
pub(crate) struct LiveAwaitingStop {
    pub(crate) session_id: DebugSessionId,
    pub(crate) adapter_type: String,
    pub(crate) is_fake: bool,
    pub(crate) thread_id: u64,
    /// Kept alive while continue worker holds the child (C4 job object).
    pub(crate) sandbox_guard: Option<legion_sandbox::spawn_stdio::PlatformGuard>,
    pub(crate) rx: std::sync::mpsc::Receiver<(
        LiveDapSession,
        Result<legion_debug::LiveDapStopOutcome, String>,
    )>,
}

pub(crate) struct DebugWorkflow {
    pub(crate) projection: DebugProjection,
    pub(crate) runtime_enabled: bool,
    pub(crate) runtime: DapClientRuntime,
    pub(crate) configurations: Vec<DebugLaunchConfiguration>,
    pub(crate) breakpoints: Vec<DebugBreakpointRecord>,
    pub(crate) pending_breakpoint_deletes: Vec<(WorkspaceId, DebugBreakpointId)>,
    pub(crate) last_audit: Option<DebugAdapterAuditRecord>,
    pub(crate) next_sequence: u64,
    pub(crate) next_watch: u64,
    /// Test seam: force live path via in-tree fake adapter when set.
    pub(crate) prefer_live_fake_for_tests: bool,
    /// Test seam: override `LEGION_DAP_MODE` without process-wide env (parallel-safe).
    pub(crate) dap_mode_for_tests: Option<legion_debug::DapMode>,
    /// Persistent live adapter session after `launch_live` (B5).
    pub(crate) live: Option<LiveDebugSession>,
    /// In-flight continue-until-stop worker (B7).
    pub(crate) awaiting_stop: Option<LiveAwaitingStop>,
    /// P2.F3.T2: authority for `debug.adapter.launch`. The workflow asks this
    /// broker before an adapter binary is resolved — the capability the denial
    /// message cites is a real decision, not a label.
    pub(crate) security_broker: DenyByDefaultBroker,
}

#[derive(Debug)]
pub(crate) struct DebugBreakpointToggleInput {
    pub(crate) context: ActiveWorkspaceContext,
    pub(crate) metadata: ActiveFileMetadata,
    pub(crate) line: u32,
    pub(crate) condition: Option<String>,
    pub(crate) hit_condition: Option<String>,
    pub(crate) log_message: Option<String>,
    pub(crate) event_context: EventContext,
}

impl Default for DebugWorkflow {
    fn default() -> Self {
        Self {
            projection: DebugProjection::empty(),
            runtime_enabled: false,
            runtime: DapClientRuntime::new(DapClientConfig::default()),
            configurations: Vec::new(),
            breakpoints: Vec::new(),
            pending_breakpoint_deletes: Vec::new(),
            last_audit: None,
            next_sequence: 0,
            next_watch: 0,
            prefer_live_fake_for_tests: false,
            dap_mode_for_tests: None,
            live: None,
            awaiting_stop: None,
            security_broker: DenyByDefaultBroker::new(
                SecurityPolicy::default(),
                CapabilityNamespace("app.debug".to_string()),
            ),
        }
    }
}

impl DebugWorkflow {
    pub(crate) fn projection(&self) -> DebugProjection {
        self.projection.clone()
    }

    pub(crate) fn effective_dap_mode(&self) -> legion_debug::DapMode {
        self.dap_mode_for_tests
            .unwrap_or_else(legion_debug::DapMode::from_env)
    }

    pub(crate) fn enable_runtime(&mut self) {
        self.runtime_enabled = true;
        self.runtime = DapClientRuntime::new(DapClientConfig::enabled());
        self.projection.live_adapter = false;
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Idle,
            message: "Debug runtime enabled (auto requires a live adapter; fixture is explicit)"
                .to_string(),
        };
        self.projection.generated_at = TimestampMillis::now();
    }

    /// Enable the DAP runtime for a product launch. Idempotent.
    ///
    /// Deliberately *not* called on workspace open: enabling on first explicit
    /// launch keeps the adapter runtime out of the way of anyone who never asks
    /// to debug, and leaves the trust decision in front of it.
    pub(crate) fn ensure_product_enabled(&mut self) {
        if !self.runtime_enabled {
            self.enable_runtime();
        }
    }

    pub(crate) fn enable_live_fake_for_tests(&mut self) {
        self.enable_runtime();
        self.prefer_live_fake_for_tests = true;
        // The CI fake is not allowlisted by the shipped policy, so the seam has
        // to widen policy explicitly. Nothing in the product path does this.
        self.security_broker
            .policy
            .debug_adapter_policy
            .allowed_adapter_binaries
            .push(FAKE_DAP_ADAPTER_BINARY.to_string());
    }

    pub(crate) fn set_dap_mode_for_tests(&mut self, mode: legion_debug::DapMode) {
        self.dap_mode_for_tests = Some(mode);
    }

    pub(crate) fn clear_workspace_state(&mut self) {
        self.drop_live_session();
        self.projection = DebugProjection::empty();
        self.configurations.clear();
        self.breakpoints.clear();
        self.pending_breakpoint_deletes.clear();
        self.last_audit = None;
        self.next_sequence = 0;
        self.next_watch = 0;
        self.runtime = if self.runtime_enabled {
            DapClientRuntime::new(DapClientConfig::enabled())
        } else {
            DapClientRuntime::new(DapClientConfig::default())
        };
    }

    pub(crate) fn drop_live_session(&mut self) {
        // Drop awaiting receiver first: worker thread still owns the session and
        // will Drop it (killing the child) when continue_until_stopped returns.
        self.awaiting_stop = None;
        if let Some(live) = self.live.take() {
            let _ = live
                .session
                .disconnect_and_wait(std::time::Duration::from_secs(2));
        }
    }

    pub(crate) fn take_last_audit(&mut self) -> Option<DebugAdapterAuditRecord> {
        self.last_audit.take()
    }

    pub(crate) fn take_pending_breakpoint_deletes(
        &mut self,
    ) -> Vec<(WorkspaceId, DebugBreakpointId)> {
        std::mem::take(&mut self.pending_breakpoint_deletes)
    }

    pub(crate) fn restore_breakpoints(&mut self, records: Vec<DebugBreakpointRecord>) {
        self.breakpoints = records
            .into_iter()
            .map(|mut record| {
                record.session_id = None;
                record
            })
            .collect();
        self.sync_breakpoint_projection();
    }

    pub(crate) fn refresh_configurations(
        &mut self,
        context: ActiveWorkspaceContext,
        root_path: &Path,
    ) -> Result<DebugProjection, AppCompositionError> {
        let mut configs =
            discover_cargo_debug_configurations(root_path, CargoDebugLocatorOptions::default())
                .map_err(debug_locator_error)?;
        for config in &mut configs {
            config.workspace_id = context.workspace_id;
            config.cwd = CanonicalPath(root_path.to_string_lossy().replace('\\', "/"));
        }
        self.configurations = configs;
        self.projection.configurations = self
            .configurations
            .iter()
            .map(debug_configuration_projection)
            .collect();
        self.sync_breakpoint_projection();
        self.projection.status = DebugStatusProjection {
            kind: self.non_lifecycle_status_kind(),
            message: format!(
                "Debug configurations refreshed: {}",
                self.projection.configurations.len()
            ),
        };
        self.projection.generated_at = TimestampMillis::now();
        Ok(self.projection())
    }

    /// Status kind to report for an action that does not change session state.
    ///
    /// Toggling a breakpoint and refreshing the configuration list are not
    /// lifecycle events. Reporting `Idle` for them while a session is active
    /// told the panel to render `status=Idle session=Some(…) state=Paused` —
    /// three fields, two of which contradict the first. On the live path it is
    /// worse than untidy: `F9` during a running program announced that the
    /// debugger had stopped while the adapter process was still executing it.
    pub(crate) fn non_lifecycle_status_kind(&self) -> DebugStatusKindProjection {
        if self.projection.active_session_id.is_some() {
            self.projection.status.kind
        } else {
            DebugStatusKindProjection::Idle
        }
    }

    pub(crate) fn toggle_breakpoint(
        &mut self,
        input: DebugBreakpointToggleInput,
    ) -> DebugProjection {
        let breakpoint_id = DebugBreakpointId(format!(
            "bp:{}:{}:{}",
            input.context.workspace_id.0, input.metadata.identity.file_id.0, input.line
        ));
        if let Some(existing) = self
            .breakpoints
            .iter()
            .position(|breakpoint| breakpoint.breakpoint_id == breakpoint_id)
        {
            let removed = self.breakpoints.remove(existing);
            self.pending_breakpoint_deletes
                .push((removed.workspace_id, removed.breakpoint_id));
            self.projection.status = DebugStatusProjection {
                kind: self.non_lifecycle_status_kind(),
                message: "Debug breakpoint removed".to_string(),
            };
        } else {
            let sequence = self.next_event_sequence();
            self.breakpoints.push(DebugBreakpointRecord {
                breakpoint_id,
                workspace_id: input.context.workspace_id,
                session_id: None,
                path: input.metadata.identity.canonical_path,
                range: ProtocolTextRange {
                    start: TextCoordinate {
                        line: input.line,
                        character: 0,
                        byte_offset: None,
                        utf16_offset: None,
                    },
                    end: TextCoordinate {
                        line: input.line,
                        character: 0,
                        byte_offset: None,
                        utf16_offset: None,
                    },
                },
                enabled: true,
                condition: input.condition,
                hit_condition: input.hit_condition,
                log_message: input.log_message,
                verified: false,
                message: Some("pending adapter verification".to_string()),
                correlation_id: input.event_context.correlation_id,
                causality_id: input.event_context.causality_id,
                sequence,
                schema_version: 1,
            });
            self.projection.status = DebugStatusProjection {
                kind: self.non_lifecycle_status_kind(),
                message: "Debug breakpoint added".to_string(),
            };
        }
        self.sync_breakpoint_projection();
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn launch(
        &mut self,
        context: ActiveWorkspaceContext,
        configuration_id: DebugConfigurationId,
        event_context: EventContext,
    ) -> DebugProjection {
        // Trust gate (B3, brokered in P2.F3.T2): untrusted workspaces never
        // spawn adapters, and the decision is what authorizes resolution below.
        let decision = self.adapter_launch_decision(&context, event_context);
        if !decision.granted {
            // Surface the broker's own reason: "requires a trusted workspace" is
            // only one of the ways this can deny, and a fixed string would
            // misreport the others (empty allowlist, unknown trust).
            return self.deny(format!(
                "{DEBUG_ADAPTER_LAUNCH_CAPABILITY} denied: {}",
                decision
                    .reason
                    .clone()
                    .unwrap_or_else(|| "requires a trusted workspace".to_string())
            ));
        }
        // Product gate: the broker above has already decided this workspace may
        // launch an adapter, so an explicit launch enables the runtime here —
        // the same lazy, trust-gated shape `TerminalWorkflow` uses.
        //
        // Until this line existed, `runtime_enabled` was set by exactly two
        // callers, `enable_debug_fixture_for_tests` and
        // `enable_debug_live_fake_for_tests`, both test seams. The shipped app
        // therefore had no path that could ever set it, so every Launch — from
        // the toolbar button, from `F5`, from `:debug-launch` — returned
        // `Denied: Debug runtime is disabled`. Checklist rows 9-12 were not
        // merely unexercised; they were unreachable.
        self.ensure_product_enabled();
        let Some(config) = self
            .configurations
            .iter()
            .find(|config| {
                config.configuration_id == configuration_id
                    && config.workspace_id == context.workspace_id
            })
            .cloned()
        else {
            return self.fail(format!(
                "debug configuration {} was not found",
                configuration_id.0
            ));
        };

        // Live path when an adapter resolves (or tests force fake).
        // Wire is Microsoft DAP (B4); resolution: LEGION_DAP_ADAPTER, PATH, or USE_FAKE.
        let mode = self.effective_dap_mode();
        if mode.allows_live() {
            match self.resolve_adapter_for_launch(&decision, &config.adapter_type) {
                Some(resolved) => {
                    match self.launch_live(&context, &config, &resolved, event_context) {
                        Ok(projection) => return projection,
                        Err(message) => {
                            if mode.require_live() {
                                return self.fail(format!(
                                    "live DAP required (LEGION_DAP_MODE=live) but launch failed: {message}"
                                ));
                            }
                            return self.fail(format!(
                                "debug adapter launch failed; no session started: {message}. Install lldb-dap or codelldb, or set LEGION_DAP_ADAPTER"
                            ));
                        }
                    }
                }
                None if mode.require_live() => {
                    return self.fail(
                        "live DAP required (LEGION_DAP_MODE=live) but no adapter resolved \
                         (set LEGION_DAP_ADAPTER, install lldb-dap/codelldb on PATH, or LEGION_DAP_USE_FAKE=1)"
                            .to_string(),
                    );
                }
                None => {
                    return self.fail(format!(
                        "no debug adapter found for {}; install lldb-dap or codelldb, set LEGION_DAP_ADAPTER, or choose fixture mode for tests",
                        config.adapter_type
                    ));
                }
            }
        }

        let breakpoints = self
            .breakpoints
            .iter()
            .filter(|breakpoint| breakpoint.workspace_id == context.workspace_id)
            .cloned()
            .map(|mut breakpoint| {
                breakpoint.session_id = None;
                breakpoint
            })
            .collect();
        let request = DebugAdapterLaunchRequest {
            workspace_id: context.workspace_id,
            configuration_id,
            adapter_type: config.adapter_type.clone(),
            breakpoints,
            schema_version: 1,
        };
        match self.runtime.launch(request) {
            Ok(outcome) => {
                self.apply_runtime_outcome(outcome);
                self.projection.live_adapter = false;
                self.projection.status = DebugStatusProjection {
                    kind: DebugStatusKindProjection::Paused,
                    message: "Simulated DAP paused (fixture; no real adapter process)".to_string(),
                };
                self.projection.session_state = Some(DebugSessionState::Paused);
                self.projection.generated_at = TimestampMillis::now();
                self.record_audit(
                    self.projection
                        .active_session_id
                        .clone()
                        .unwrap_or_else(|| DebugSessionId("debug:missing".to_string())),
                    DebugSessionState::Paused,
                    config.adapter_type,
                    event_context,
                    "action=launch state=paused simulated=true".to_string(),
                );
                self.projection()
            }
            Err(error) => self.fail(format!("debug launch denied: {error}")),
        }
    }

    /// Ask the broker whether this workspace may launch a debug adapter.
    ///
    /// Runs before resolution, not after: a denied workspace must not even learn
    /// whether an adapter binary is installed.
    pub(crate) fn adapter_launch_decision(
        &self,
        context: &ActiveWorkspaceContext,
        event_context: EventContext,
    ) -> CapabilityDecision {
        let denied = |reason: String| CapabilityDecision {
            decision_id: CapabilityDecisionId(1),
            granted: false,
            capability: CapabilityId(DEBUG_ADAPTER_LAUNCH_CAPABILITY.to_string()),
            reason: Some(reason),
        };
        match self.security_broker.handle(CapabilityRequest::Request {
            principal_id: context.principal.clone(),
            capability_id: CapabilityId(DEBUG_ADAPTER_LAUNCH_CAPABILITY.to_string()),
            workspace_trust_state: context.trust.clone(),
            target_path: None,
            decision_id: None,
            context: CapabilityRequestContext::default(),
            correlation_id: event_context.correlation_id,
        }) {
            Ok(CapabilityResponse::Decision(decision)) => decision,
            Ok(other) => denied(format!(
                "debug policy returned unexpected response: {other:?}"
            )),
            Err(error) => denied(format!("debug policy request failed: {error:?}")),
        }
    }

    /// Mint the resolution grant from a granted decision.
    ///
    /// Returns [`None`] whenever policy would refuse, so a caller holding a
    /// denied decision cannot reach [`resolve_live_adapter`] at all.
    pub(crate) fn adapter_resolution_grant(
        &self,
        decision: &CapabilityDecision,
    ) -> Option<legion_debug::AdapterResolutionGrant> {
        legion_debug::AdapterResolutionGrant::from_decision(
            decision,
            &self
                .security_broker
                .policy
                .debug_adapter_policy
                .allowed_adapter_binaries,
        )
    }

    pub(crate) fn resolve_adapter_for_launch(
        &self,
        decision: &CapabilityDecision,
        preferred_type: &str,
    ) -> Option<legion_debug::ResolvedAdapter> {
        let grant = self.adapter_resolution_grant(decision)?;
        if self.prefer_live_fake_for_tests {
            // The test seam builds its own ResolvedAdapter, so it has to run the
            // program past the same allowlist or it would be a hole in the gate.
            return legion_debug::fake_dap_adapter_path()
                .filter(|program| grant.permits_program(program))
                .map(|program| legion_debug::ResolvedAdapter {
                    program,
                    args: Vec::new(),
                    adapter_type: "legion-fake".to_string(),
                    is_fake: true,
                });
        }
        resolve_live_adapter(&grant, preferred_type)
    }

    pub(crate) fn launch_live(
        &mut self,
        context: &ActiveWorkspaceContext,
        config: &DebugLaunchConfiguration,
        resolved: &legion_debug::ResolvedAdapter,
        event_context: EventContext,
    ) -> Result<DebugProjection, String> {
        use std::time::Duration;

        // B12: system adapters need a real binary; run cargo prebuild when
        // configuration carries cargo_args. Fake adapter skips (CI speed).
        let prebuild_note = if live_dap_should_prebuild_impl(resolved.is_fake, &config.cargo_args) {
            Some(run_live_dap_prebuild_impl(
                config.cwd.0.as_str(),
                &config.cargo_args,
                Duration::from_secs(180),
            )?)
        } else {
            None
        };

        let (mut session, sandbox_guard, sandbox_note) =
            spawn_live_dap_session(resolved, config.cwd.0.as_str())?;
        let handshake = session
            .initialize_handshake(Duration::from_secs(5))
            .map_err(|err| err.to_string())?;

        // DAP setBreakpoints is per-source: group by path, then map responses.
        let mut by_path: std::collections::BTreeMap<String, Vec<usize>> =
            std::collections::BTreeMap::new();
        for (index, bp) in self.breakpoints.iter().enumerate() {
            if bp.workspace_id == context.workspace_id {
                by_path.entry(bp.path.0.clone()).or_default().push(index);
            }
        }
        for (path, indices) in by_path {
            let lines: Vec<u64> = indices
                .iter()
                .map(|&i| u64::from(self.breakpoints[i].range.start.line.saturating_add(1)))
                .collect();
            let verified = session
                .set_breakpoints(&path, &lines, Duration::from_secs(3))
                .map_err(|err| err.to_string())?;
            for (idx, verified_bp) in indices.into_iter().zip(verified) {
                if let Some(bp) = self.breakpoints.get_mut(idx) {
                    bp.verified = verified_bp.verified;
                    bp.message = verified_bp.message.clone();
                }
            }
        }
        if !self.breakpoints.is_empty() {
            self.sync_breakpoint_projection();
        }

        // Resolve relative program labels against configuration cwd (workspace root).
        let program = {
            let label = config.program_label.clone();
            let candidate = Path::new(&label);
            if candidate.is_absolute() {
                label
            } else {
                Path::new(config.cwd.0.as_str())
                    .join(candidate)
                    .to_string_lossy()
                    .replace('\\', "/")
            }
        };
        let stop = session
            .launch_until_stopped_with(
                &program,
                Some(config.cwd.0.as_str()),
                config.stop_on_entry,
                Duration::from_secs(5),
            )
            .map_err(|err| err.to_string())?;

        // B5: keep the adapter process alive for step/continue (do not disconnect).
        self.drop_live_session();
        let session_id = DebugSessionId(format!(
            "dap-live:{}:{}",
            context.workspace_id.0, config.configuration_id.0
        ));
        let thread_id = stop.thread_id;
        self.live = Some(LiveDebugSession {
            session,
            thread_id,
            session_id: session_id.clone(),
            adapter_type: resolved.adapter_type.clone(),
            is_fake: resolved.is_fake,
            _sandbox_guard: sandbox_guard,
        });

        self.projection.active_session_id = Some(session_id.clone());
        self.projection.session_state = Some(DebugSessionState::Paused);
        self.projection.live_adapter = true;
        self.apply_live_stop(&session_id, &stop);
        if let Some(note) = prebuild_note {
            self.projection.console.push(DebugConsoleProjection {
                session_id: session_id.clone(),
                category_label: "adapter".to_string(),
                message_label: bounded_label(format!("LIVE DAP prebuild: {note}"), 160),
            });
        }
        if let Some(note) = sandbox_note {
            self.projection.console.push(DebugConsoleProjection {
                session_id: session_id.clone(),
                category_label: "adapter".to_string(),
                message_label: bounded_label(format!("LIVE DAP sandbox: {note}"), 160),
            });
        }
        self.projection.console.push(DebugConsoleProjection {
            session_id: session_id.clone(),
            category_label: "adapter".to_string(),
            message_label: bounded_label(
                format!(
                    "LIVE DAP: initialize adapter={} fake={} persistent=true • {}",
                    resolved.adapter_type, resolved.is_fake, handshake.metadata_summary
                ),
                160,
            ),
        });
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Paused,
            message: format!(
                "Live DAP paused (adapter={} fake={} reason={} persistent=true)",
                resolved.adapter_type, resolved.is_fake, stop.reason
            ),
        };
        self.projection.generated_at = TimestampMillis::now();
        self.record_audit(
            session_id,
            DebugSessionState::Paused,
            resolved.adapter_type.clone(),
            event_context,
            stop.metadata_summary,
        );
        Ok(self.projection())
    }

    pub(crate) fn apply_live_stop(
        &mut self,
        session_id: &DebugSessionId,
        stop: &legion_debug::LiveDapStopOutcome,
    ) {
        self.projection.stack_frames = stop
            .stack_frames
            .iter()
            .map(|frame| DebugStackFrameProjection {
                session_id: session_id.clone(),
                frame_id: frame.id,
                name: frame.name.clone(),
                path: frame.path.as_ref().map(|p| CanonicalPath(p.clone())),
                line: Some(frame.line as u32),
            })
            .collect();
        self.projection.variables = stop
            .variables
            .iter()
            .map(|var| DebugVariableProjection {
                session_id: session_id.clone(),
                name: var.name.clone(),
                value_label: var.value.clone(),
                type_label: var.type_label.clone(),
                has_children: false,
            })
            .collect();
        if let Some(live) = self.live.as_mut() {
            live.thread_id = stop.thread_id;
        }
    }

    pub(crate) fn step(
        &mut self,
        session_id: DebugSessionId,
        kind: DebugStepKindProjection,
    ) -> DebugProjection {
        if !self.session_is_active(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        if self
            .live
            .as_ref()
            .is_some_and(|live| live.session_id == session_id)
        {
            return self.step_live(session_id, kind);
        }
        let protocol_kind = debug_step_kind(kind);
        match self.runtime.step(session_id, protocol_kind) {
            Ok(outcome) => {
                self.apply_runtime_outcome(outcome);
                self.projection.status = DebugStatusProjection {
                    kind: DebugStatusKindProjection::Paused,
                    message: "Simulated DAP step completed (fixture)".to_string(),
                };
                self.projection.session_state = Some(DebugSessionState::Paused);
                self.projection.generated_at = TimestampMillis::now();
                self.projection()
            }
            Err(error) => self.fail(format!("debug step failed: {error}")),
        }
    }

    pub(crate) fn step_live(
        &mut self,
        session_id: DebugSessionId,
        kind: DebugStepKindProjection,
    ) -> DebugProjection {
        use legion_ui::DebugStepKindProjection as StepKind;
        use std::time::Duration;

        let (thread_id, adapter_type, is_fake) = {
            let Some(live) = self.live.as_ref() else {
                return self.fail("live DAP session missing".to_string());
            };
            (live.thread_id, live.adapter_type.clone(), live.is_fake)
        };
        let timeout = Duration::from_secs(5);

        if self.awaiting_stop.is_some() {
            return self.deny(
                "live DAP continue is in progress; use :debug-poll or :debug-stop".to_string(),
            );
        }

        match kind {
            StepKind::Continue => {
                // B7: non-blocking continue — return Running immediately; poll for stop.
                let Some(live) = self.live.take() else {
                    return self.fail("live DAP session missing".to_string());
                };
                let (tx, rx) = std::sync::mpsc::channel();
                let wait_thread_id = live.thread_id;
                let wait_timeout = Duration::from_secs(30);
                // Keep sandbox guard alive while worker holds the child (C4).
                let sandbox_guard = live._sandbox_guard;
                std::thread::spawn(move || {
                    let mut session = live.session;
                    let result = session
                        .continue_until_stopped(wait_thread_id, wait_timeout)
                        .map_err(|err| err.to_string());
                    let _ = tx.send((session, result));
                });
                self.awaiting_stop = Some(LiveAwaitingStop {
                    session_id: session_id.clone(),
                    adapter_type: adapter_type.clone(),
                    is_fake,
                    thread_id,
                    sandbox_guard,
                    rx,
                });
                self.projection.session_state = Some(DebugSessionState::Running);
                self.projection.live_adapter = true;
                self.projection.stack_frames.clear();
                self.projection.variables.clear();
                self.projection.status = DebugStatusProjection {
                    kind: DebugStatusKindProjection::Running,
                    message: format!(
                        "Live DAP continuing (adapter={adapter_type} fake={is_fake}; poll for stop)"
                    ),
                };
                self.projection.console.push(DebugConsoleProjection {
                    session_id: session_id.clone(),
                    category_label: "adapter".to_string(),
                    message_label: "LIVE DAP: continue started (non-blocking; use :debug-poll)"
                        .to_string(),
                });
                self.projection.generated_at = TimestampMillis::now();
                self.projection()
            }
            StepKind::Back => {
                self.fail("live DAP reverse-step is not supported on this adapter path".to_string())
            }
            StepKind::Over | StepKind::Into | StepKind::Out => {
                let command = match kind {
                    StepKind::Over => "next",
                    StepKind::Into => "stepIn",
                    StepKind::Out => "stepOut",
                    _ => unreachable!(),
                };
                let stepped = self.live.as_mut().map(|live| {
                    live.session
                        .step_command_until_stopped(command, thread_id, timeout)
                });
                match stepped {
                    Some(Ok(stop)) => {
                        self.apply_live_stop(&session_id, &stop);
                        self.projection.session_state = Some(DebugSessionState::Paused);
                        self.projection.live_adapter = true;
                        self.projection.status = DebugStatusProjection {
                            kind: DebugStatusKindProjection::Paused,
                            message: format!(
                                "Live DAP {command} paused (adapter={adapter_type} fake={is_fake} reason={})",
                                stop.reason
                            ),
                        };
                        self.projection.console.push(DebugConsoleProjection {
                            session_id: session_id.clone(),
                            category_label: "adapter".to_string(),
                            message_label: bounded_label(
                                format!("LIVE DAP step: {} • {}", command, stop.metadata_summary),
                                160,
                            ),
                        });
                        self.projection.generated_at = TimestampMillis::now();
                        self.projection()
                    }
                    Some(Err(err)) => self.fail(format!("live DAP {command} failed: {err}")),
                    None => self.fail("live DAP session missing".to_string()),
                }
            }
        }
    }

    /// Poll for a stop after non-blocking continue (B7).
    pub(crate) fn poll_session(&mut self, session_id: DebugSessionId) -> DebugProjection {
        if self.projection.active_session_id.as_ref() != Some(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        let Some(awaiting) = self.awaiting_stop.as_ref() else {
            // Nothing to poll; return current projection (paused or running fixture).
            return self.projection();
        };
        if awaiting.session_id != session_id {
            return self.deny("debug poll session mismatch".to_string());
        }
        match awaiting.rx.try_recv() {
            Ok((session, Ok(stop))) => {
                let meta = self
                    .awaiting_stop
                    .take()
                    .expect("awaiting_stop present after try_recv");
                self.live = Some(LiveDebugSession {
                    session,
                    thread_id: stop.thread_id,
                    session_id: meta.session_id.clone(),
                    adapter_type: meta.adapter_type.clone(),
                    is_fake: meta.is_fake,
                    _sandbox_guard: meta.sandbox_guard,
                });
                self.apply_live_stop(&session_id, &stop);
                self.projection.session_state = Some(DebugSessionState::Paused);
                self.projection.live_adapter = true;
                self.projection.status = DebugStatusProjection {
                    kind: DebugStatusKindProjection::Paused,
                    message: format!(
                        "Live DAP continued then stopped (adapter={} fake={} reason={})",
                        meta.adapter_type, meta.is_fake, stop.reason
                    ),
                };
                self.projection.console.push(DebugConsoleProjection {
                    session_id: session_id.clone(),
                    category_label: "adapter".to_string(),
                    message_label: bounded_label(
                        format!(
                            "LIVE DAP continue→stop (poll): reason={} • {}",
                            stop.reason, stop.metadata_summary
                        ),
                        160,
                    ),
                });
                self.projection.generated_at = TimestampMillis::now();
                self.projection()
            }
            Ok((session, Err(err))) => {
                let meta = self
                    .awaiting_stop
                    .take()
                    .expect("awaiting_stop present after try_recv");
                self.live = Some(LiveDebugSession {
                    session,
                    thread_id: meta.thread_id,
                    session_id: meta.session_id,
                    adapter_type: meta.adapter_type,
                    is_fake: meta.is_fake,
                    _sandbox_guard: meta.sandbox_guard,
                });
                self.fail(format!("live DAP continue failed: {err}"))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.projection.session_state = Some(DebugSessionState::Running);
                self.projection.live_adapter = true;
                self.projection.status = DebugStatusProjection {
                    kind: DebugStatusKindProjection::Running,
                    message: "Live DAP still running (poll again)".to_string(),
                };
                self.projection.generated_at = TimestampMillis::now();
                self.projection()
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.awaiting_stop = None;
                self.fail("live DAP continue worker disconnected".to_string())
            }
        }
    }

    pub(crate) fn stop_session(&mut self, session_id: DebugSessionId) -> DebugProjection {
        if !self.session_is_active(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        let was_live = self
            .live
            .as_ref()
            .is_some_and(|live| live.session_id == session_id)
            || self
                .awaiting_stop
                .as_ref()
                .is_some_and(|awaiting| awaiting.session_id == session_id);
        // Always clear live + awaiting (awaiting drop abandons worker; session Drop kills child).
        self.drop_live_session();
        self.projection.active_session_id = None;
        self.projection.session_state = Some(DebugSessionState::Exited);
        self.projection.live_adapter = false;
        self.projection.stack_frames.clear();
        self.projection.variables.clear();
        self.projection.inline_values.clear();
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Exited,
            message: if was_live {
                "Live DAP session disconnected".to_string()
            } else {
                "Debug session stopped (fixture)".to_string()
            },
        };
        self.projection.console.push(DebugConsoleProjection {
            session_id: session_id.clone(),
            category_label: "adapter".to_string(),
            message_label: if was_live {
                "LIVE DAP: disconnect".to_string()
            } else {
                "SIMULATED DAP: session stopped".to_string()
            },
        });
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn run_to_cursor(
        &mut self,
        session_id: DebugSessionId,
        buffer_id: BufferId,
        position: TextCoordinate,
    ) -> DebugProjection {
        if !self.session_is_active(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        self.push_console(
            session_id,
            format!(
                "run-to-cursor buffer={} line={} character={}",
                buffer_id.0, position.line, position.character
            ),
            DebugConsoleCategory::Adapter,
        );
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Paused,
            message: "Debug run-to-cursor completed".to_string(),
        };
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn evaluate_selection(
        &mut self,
        session_id: DebugSessionId,
        expression_label: String,
    ) -> DebugProjection {
        if !self.session_is_active(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        self.push_console(
            session_id,
            format!("evaluate expression_bytes={}", expression_label.len()),
            DebugConsoleCategory::Evaluation,
        );
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn add_watch(
        &mut self,
        session_id: DebugSessionId,
        expression_label: String,
    ) -> DebugProjection {
        if !self.session_is_active(&session_id) {
            return self.deny(format!("debug session {} is not active", session_id.0));
        }
        self.next_watch = self.next_watch.saturating_add(1).max(1);
        self.projection.watches.push(DebugWatchProjection {
            watch_id: DebugWatchId(format!("watch-{}", self.next_watch)),
            session_id: session_id.clone(),
            expression_label: bounded_label(expression_label, 80),
            value_label: "metadata-only".to_string(),
            type_label: Some("debug".to_string()),
        });
        self.push_console(
            session_id,
            "watch added value=metadata-only".to_string(),
            DebugConsoleCategory::Evaluation,
        );
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn apply_runtime_outcome(&mut self, outcome: DapClientOutcome) {
        self.last_audit = Some(outcome.audit.clone());
        self.projection.live_adapter = false;
        self.projection.active_session_id = Some(outcome.audit.session_id.clone());
        for verified in outcome.breakpoints {
            if let Some(existing) = self
                .breakpoints
                .iter_mut()
                .find(|breakpoint| breakpoint.breakpoint_id == verified.breakpoint_id)
            {
                existing.verified = verified.verified;
                existing.message = verified.message;
                existing.session_id = None;
            }
        }
        self.sync_breakpoint_projection();
        self.projection.stack_frames = outcome
            .stack_frames
            .into_iter()
            .map(debug_stack_frame_projection)
            .collect();
        self.projection.variables = outcome
            .variables
            .into_iter()
            .map(debug_variable_projection)
            .collect();
        self.projection.inline_values = outcome
            .inline_values
            .into_iter()
            .map(debug_inline_value_projection)
            .collect();
        self.projection
            .console
            .extend(outcome.console.into_iter().map(debug_console_projection));
    }

    pub(crate) fn sync_breakpoint_projection(&mut self) {
        self.breakpoints.sort_by(|left, right| {
            (
                left.path.0.as_str(),
                left.range.start.line,
                left.breakpoint_id.0.as_str(),
            )
                .cmp(&(
                    right.path.0.as_str(),
                    right.range.start.line,
                    right.breakpoint_id.0.as_str(),
                ))
        });
        self.projection.breakpoints = self
            .breakpoints
            .iter()
            .map(debug_breakpoint_projection)
            .collect();
    }

    pub(crate) fn deny(&mut self, reason: String) -> DebugProjection {
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Denied,
            message: reason.clone(),
        };
        self.projection.diagnostics.push(bounded_label(reason, 120));
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn fail(&mut self, reason: String) -> DebugProjection {
        self.projection.status = DebugStatusProjection {
            kind: DebugStatusKindProjection::Failed,
            message: reason.clone(),
        };
        self.projection.diagnostics.push(bounded_label(reason, 120));
        self.projection.generated_at = TimestampMillis::now();
        self.projection()
    }

    pub(crate) fn push_console(
        &mut self,
        session_id: DebugSessionId,
        message_label: String,
        category: DebugConsoleCategory,
    ) {
        self.projection.console.push(DebugConsoleProjection {
            session_id,
            category_label: debug_console_category_label(category).to_string(),
            message_label: bounded_label(message_label, 160),
        });
        if self.projection.console.len() > 100 {
            let excess = self.projection.console.len() - 100;
            self.projection.console.drain(0..excess);
        }
    }

    pub(crate) fn session_is_active(&self, session_id: &DebugSessionId) -> bool {
        self.projection.active_session_id.as_ref() == Some(session_id)
            && self.projection.session_state.is_some()
    }

    pub(crate) fn record_audit(
        &mut self,
        session_id: DebugSessionId,
        state: DebugSessionState,
        adapter_type: String,
        event_context: EventContext,
        metadata_summary: String,
    ) {
        self.last_audit = Some(DebugAdapterAuditRecord {
            session_id,
            state,
            adapter_type,
            event_sequence: self.next_event_sequence(),
            correlation_id: event_context.correlation_id,
            causality_id: event_context.causality_id,
            metadata_summary: bounded_label(metadata_summary, 160),
            redaction_hints: vec![RedactionHint::MetadataOnly],
            schema_version: 1,
        });
    }

    pub(crate) fn next_event_sequence(&mut self) -> EventSequence {
        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
        EventSequence(self.next_sequence)
    }
}

/// C4: spawn live DAP session — sandboxed stdio for non-fake adapters.
fn spawn_live_dap_session(
    resolved: &legion_debug::ResolvedAdapter,
    workspace_cwd: &str,
) -> Result<
    (
        LiveDapSession,
        Option<legion_sandbox::spawn_stdio::PlatformGuard>,
        Option<String>,
    ),
    String,
> {
    if resolved.is_fake {
        let session = LiveDapSession::spawn(
            &resolved.program,
            &resolved.args,
            resolved.adapter_type.as_str(),
        )
        .map_err(|err| err.to_string())?;
        return Ok((session, None, None));
    }

    use legion_sandbox::spawn_stdio::{SandboxStdioSpec, spawn_sandboxed_stdio};
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    let cwd = PathBuf::from(workspace_cwd);
    let spec = SandboxStdioSpec {
        program: resolved.program.clone(),
        args: resolved.args.clone(),
        working_dir: cwd.clone(),
        writable_root: cwd,
        allowed_egress: BTreeSet::new(),
        env: Vec::new(),
    };
    match spawn_sandboxed_stdio(&spec) {
        Ok(sandboxed) => {
            let (child, stdin, stdout, report, guard) = sandboxed.into_parts();
            let note = format!(
                "backend={} fs_write={} net={} caveats={}",
                report.backend_used,
                report.filesystem_write_enforced,
                report.network_enforced,
                if report.caveat_labels.is_empty() {
                    "none".to_string()
                } else {
                    report.caveat_labels.join(",")
                }
            );
            let session =
                LiveDapSession::from_stdio(child, stdin, stdout, resolved.adapter_type.as_str())
                    .map_err(|err| err.to_string())?;
            Ok((session, Some(guard), Some(note)))
        }
        Err(err) => Err(format!(
            "refusing to launch live DAP adapter without sandbox enforcement: {err}"
        )),
    }
}

/// B12: whether live launch should run a cargo prebuild.
///
/// Exposed under `test-helpers` for integration tests; production callers use
/// `launch_live` only.
#[cfg(any(test, feature = "test-helpers"))]
pub fn live_dap_should_prebuild(is_fake: bool, cargo_args: &[String]) -> bool {
    live_dap_should_prebuild_impl(is_fake, cargo_args)
}

fn live_dap_should_prebuild_impl(is_fake: bool, cargo_args: &[String]) -> bool {
    !is_fake && !cargo_args.is_empty()
}

/// Execute `cargo <cargo_args>` in `cwd` with a hard timeout (B12).
///
/// Returns a short metadata-only summary for the debug console (no full logs).
/// Stdio is nulled so a chatty cargo cannot fill pipes and deadlock the wait.
#[cfg(any(test, feature = "test-helpers"))]
pub fn run_live_dap_prebuild(
    cwd: &str,
    cargo_args: &[String],
    timeout: std::time::Duration,
) -> Result<String, String> {
    run_live_dap_prebuild_impl(cwd, cargo_args, timeout)
}

fn run_live_dap_prebuild_impl(
    cwd: &str,
    cargo_args: &[String],
    timeout: std::time::Duration,
) -> Result<String, String> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = Command::new("cargo")
        .args(cargo_args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("cargo prebuild spawn failed in {cwd}: {err}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(format!("cargo {} ok (cwd={})", cargo_args.join(" "), cwd));
                }
                return Err(format!(
                    "cargo prebuild failed (status={status}; args={})",
                    cargo_args.join(" ")
                ));
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "cargo prebuild timed out after {timeout:?} (args={})",
                        cargo_args.join(" ")
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(err) => {
                let _ = child.kill();
                return Err(format!("cargo prebuild wait failed: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod adapter_policy_tests {
    use super::*;
    use std::path::Path;

    fn decision(granted: bool, capability: &str) -> CapabilityDecision {
        CapabilityDecision {
            decision_id: CapabilityDecisionId(3),
            granted,
            capability: CapabilityId(capability.to_string()),
            reason: None,
        }
    }

    #[test]
    fn capability_id_matches_between_security_policy_and_debug_crate() {
        // The two crates cannot depend on each other, so the shared id is only
        // as good as this assertion: if either side renames it, the grant stops
        // minting and every launch silently falls back to the fixture.
        assert_eq!(
            DEBUG_ADAPTER_LAUNCH_CAPABILITY,
            legion_debug::DEBUG_ADAPTER_LAUNCH_CAPABILITY
        );
    }

    #[test]
    fn denied_decision_mints_no_resolution_grant() {
        let workflow = DebugWorkflow::default();
        assert!(
            workflow
                .adapter_resolution_grant(&decision(false, DEBUG_ADAPTER_LAUNCH_CAPABILITY))
                .is_none(),
            "a denied decision must not authorize adapter resolution"
        );
        assert!(
            workflow
                .adapter_resolution_grant(&decision(true, "terminal.launch"))
                .is_none(),
            "a grant for another capability must not authorize adapter resolution"
        );
    }

    #[test]
    fn shipped_policy_allows_rust_adapters_but_not_the_ci_fake() {
        let workflow = DebugWorkflow::default();
        let grant = workflow
            .adapter_resolution_grant(&decision(true, DEBUG_ADAPTER_LAUNCH_CAPABILITY))
            .expect("granted decision mints a grant under the shipped policy");
        assert!(grant.permits_program(Path::new("lldb-dap")));
        assert!(grant.permits_program(Path::new("/opt/codelldb/codelldb")));
        assert!(
            !grant.permits_program(Path::new("fake_dap_adapter.exe")),
            "the in-tree CI fake must not be launchable under the shipped policy"
        );
        // Written without a separator so the assertion means the same thing on
        // every platform. Off Windows a backslash is an ordinary filename
        // character, so a `C:\Windows\…` literal has no directory part and this
        // would pass for the wrong reason — proving nothing about the allowlist.
        assert!(!grant.permits_program(Path::new("cmd.exe")));
        #[cfg(windows)]
        assert!(!grant.permits_program(Path::new("C:\\Windows\\System32\\cmd.exe")));
    }

    #[test]
    fn live_fake_test_seam_widens_policy_rather_than_bypassing_it() {
        let mut workflow = DebugWorkflow::default();
        workflow.enable_live_fake_for_tests();
        let grant = workflow
            .adapter_resolution_grant(&decision(true, DEBUG_ADAPTER_LAUNCH_CAPABILITY))
            .expect("grant");
        assert!(
            grant.permits_program(Path::new("fake_dap_adapter")),
            "the seam is supposed to allowlist the fake, not skip the allowlist"
        );
        // Still a policy, not an open door.
        assert!(!grant.permits_program(Path::new("bash")));
    }
}
