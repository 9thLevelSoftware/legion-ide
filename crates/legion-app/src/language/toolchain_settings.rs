//! Local TypeScript and Python toolchain settings: configure, read, inspect
//! state, clear.
//!
//! Moved verbatim out of `lib.rs` for the chokepoint budget (cross-cutting
//! rule 1). Nothing here changed in the move: the enum, the four inherent
//! `AppComposition` methods and the approval tests are the same bytes that
//! lived in `lib.rs`, and `LanguageToolchainConfigurationState` is re-exported
//! from the crate root so `legion_app::LanguageToolchainConfigurationState`
//! keeps resolving.
//!
//! # Why the Python interpreter path is explicit
//!
//! Nothing in this module resolves an executable through `PATH`, and
//! [`AppComposition::configure_python_toolchain`] refuses a bare executable
//! name before it touches the filesystem, the policy store, or any process.
//! That refusal is the fix for a recorded defect, not a style preference: with
//! no explicit interpreter a Pyright child inherits `PATH`, and on the Windows
//! host in the ledger the first `python.exe` on `PATH` is the WindowsApps
//! `AppExecLink` stub rather than an interpreter.
//!
//! Pyright itself will not accept a bare name either. In the pinned 1.1.400
//! bundle (`.superpowers/sdd/2026-09-04-full-product-completion/`
//! `python-lsp-probe/cache/pyright-1.1.400/package/dist/pyright-internal.js`)
//! `isPythonBinary` is `(e) => "python" === e.trim() || "python3" === e.trim()`
//! and a `pythonPath` matching it is discarded, after which Pyright falls back
//! to spawning `python` itself.
//!
//! # Pyright configuration payload
//!
//! [`pyright_initialization_options`] is pure: settings in, JSON out, and
//! [`AppComposition::pyright_configuration_payload`] is the reachable accessor
//! that feeds it the configured settings (returning `None` when there are
//! none). Neither starts a process or grants anything. The key set was read out
//! of that same pinned Pyright 1.1.400 bundle, not guessed:
//!
//! - `diagnosticMode` and `disablePullDiagnostics` are the **only** two keys
//!   Pyright 1.1.400 reads out of `initializationOptions`. `initializationOptions`
//!   appears exactly once in the whole bundle, in `LanguageServerBase.initialize`,
//!   and the object it binds is read only as `?.diagnosticMode` and
//!   `?.disablePullDiagnostics`. `isOpenFilesOnly` treats every value other than
//!   `"workspace"` as open-files-only, so `"openFilesOnly"` is the explicit form
//!   of the default.
//! - `settings.python.pythonPath` is the absolute canonical interpreter path.
//!   Pyright resolves the interpreter through
//!   `getConfiguration(workspaceRootUri, "python").pythonPath`, which is answered
//!   either by a `workspace/configuration` response or, when the client declares
//!   no configuration capability, out of `defaultClientConfig` — which Pyright
//!   assigns from the `settings` member of `workspace/didChangeConfiguration`.
//!   The nested `settings` object here is therefore exactly the payload a caller
//!   replays on that notification, and `settings.python` is exactly the answer to
//!   a `workspace/configuration` request for section `python`. Pyright 1.1.400
//!   does **not** read `pythonPath` out of `initializationOptions`; carrying it
//!   in this object is a convenience for the caller, and this comment says so
//!   rather than implying the interpreter arrives at initialize time.
//!
//! No Pyright process is started from this module and none was started to
//! produce this payload. The shape is source evidence against a pinned bundle.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{AppComposition, AppCompositionError, TypeScriptBundleStartup};
use legion_protocol::{
    CanonicalPath, LanguageServerId, LanguageToolchainSettingsRecord, LanguageToolingStatusKind,
    ProtocolError, PythonToolchainSettings, TypeScriptToolchainSettings, WorkspaceTrustState,
};

#[cfg(test)]
#[path = "toolchain_approval_tests.rs"]
mod toolchain_approval_tests;

/// Non-persistent state of the normal TypeScript toolchain controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageToolchainConfigurationState {
    /// No saved operator input exists.
    Unconfigured,
    /// Saved metadata exists but has not been explicitly approved in this app
    /// session (including unsupported schema versions).
    Draft,
    /// Metadata and the app-owned runtime bindings are configured.
    Configured,
}

impl AppComposition {
    /// Configure the operator-selected local TypeScript toolchain for the
    /// active trusted workspace.
    ///
    /// This records only canonical input metadata. It does not materialize
    /// either archive or start a process; each explicit start/restart creates
    /// fresh artifact and Node receipts through the language authority.
    pub fn configure_typescript_toolchain(
        &mut self,
        server_archive: impl AsRef<Path>,
        compiler_archive: impl AsRef<Path>,
        node_executable: impl AsRef<Path>,
    ) -> Result<(), AppCompositionError> {
        if self.active_documents.workspace_id().is_none() {
            return Err(AppCompositionError::WorkspaceNotOpen);
        }
        if self.active_documents.active_workspace_trust != Some(WorkspaceTrustState::Trusted) {
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_untrusted".to_string(),
                message: "TypeScript toolchains require a trusted workspace".to_string(),
            }));
        }

        fn canonical_regular_file(
            path: &Path,
            label: &str,
        ) -> Result<PathBuf, AppCompositionError> {
            let canonical = std::fs::canonicalize(path).map_err(|error| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} is invalid: {error}"),
                })
            })?;
            if !canonical.is_file() {
                return Err(AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} must be a regular file"),
                }));
            }
            if canonical.to_str().is_none() {
                return Err(AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_input_invalid".to_string(),
                    message: format!("{label} path is not valid UTF-8"),
                }));
            }
            Ok(canonical)
        }

        // Validate every input before changing the policy store or replacing
        // the existing configuration, so an invalid third path is atomic.
        let server_archive = canonical_regular_file(server_archive.as_ref(), "server archive")?;
        let compiler_archive =
            canonical_regular_file(compiler_archive.as_ref(), "compiler archive")?;
        let node_executable = canonical_regular_file(node_executable.as_ref(), "Node executable")?;
        let root = self
            .active_documents
            .workspace_root_path
            .as_deref()
            .ok_or(AppCompositionError::WorkspaceNotOpen)?;
        let root = std::fs::canonicalize(root).map_err(|error| {
            AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_invalid".to_string(),
                message: error.to_string(),
            })
        })?;
        if !root.is_dir() {
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_invalid".to_string(),
                message: "workspace root must be a directory".to_string(),
            }));
        }
        let previous_node = self.typescript_node_approval.clone();
        self.language_startup_authority
            .allow_exact_binary(&node_executable)
            .map_err(|error| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_node_invalid".to_string(),
                    message: error.to_string(),
                })
            })?;
        self.typescript_node_approval = Some(node_executable.clone());
        let mut replaced_nodes = HashSet::new();
        for server_id in [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ] {
            if let Some(bundle) = self.typescript_bundles.get(&server_id) {
                replaced_nodes.insert(bundle.node_path.clone());
            }
            if let Some(path) = self.language_server_configured_paths.remove(&server_id) {
                replaced_nodes.insert(path);
            }
            if let Some(config) = self.language_server_local_downloads.remove(&server_id) {
                replaced_nodes.insert(config.node_path);
            }
            if let Some(config) = self.language_server_downloaded.remove(&server_id) {
                replaced_nodes.insert(config.approved_node.canonical_path().to_path_buf());
            }
        }
        let cache_root = root.join(".legion").join("language-tools");
        let descriptor = crate::language::TypeScriptBundleDescriptor::pinned();
        let settings = TypeScriptToolchainSettings {
            server_archive: CanonicalPath(server_archive.to_str().unwrap().to_string()),
            compiler_archive: CanonicalPath(compiler_archive.to_str().unwrap().to_string()),
            node_executable: CanonicalPath(node_executable.to_str().unwrap().to_string()),
        };
        self.language_toolchain_settings.schema_version = 1;
        for server_id in [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ] {
            self.typescript_bundles.insert(
                server_id,
                TypeScriptBundleStartup {
                    descriptor: descriptor.clone(),
                    server_archive: server_archive.clone(),
                    compiler_archive: compiler_archive.clone(),
                    node_path: node_executable.clone(),
                    cache_root: cache_root.clone(),
                },
            );
        }
        if let Some(previous_node) = previous_node {
            replaced_nodes.insert(previous_node);
        }
        for node in replaced_nodes {
            if node != node_executable
                && !self
                    .language_server_configured_paths
                    .values()
                    .any(|path| path == &node)
                && !self
                    .language_server_local_downloads
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
                && !self
                    .language_server_downloaded
                    .values()
                    .any(|config| config.approved_node.canonical_path() == node.as_path())
                && !self
                    .typescript_bundles
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
            {
                let _ = self.language_startup_authority.revoke_exact_binary(&node);
            }
        }
        self.language_toolchain_settings.typescript = Some(settings);
        Ok(())
    }

    /// Return the metadata-only local language-toolchain configuration.
    pub fn language_toolchain_settings(&self) -> LanguageToolchainSettingsRecord {
        self.language_toolchain_settings.clone()
    }

    /// Return the non-persistent approval/configuration state for UI
    /// projection. A restored record is always a draft until reconfigured.
    pub fn language_toolchain_configuration_state(&self) -> LanguageToolchainConfigurationState {
        if self.language_toolchain_settings.typescript.is_none() {
            LanguageToolchainConfigurationState::Unconfigured
        } else if self.language_toolchain_settings.schema_version != 1
            || ![102_u64, 106, 107, 108]
                .iter()
                .all(|id| self.typescript_bundles.contains_key(&LanguageServerId(*id)))
        {
            LanguageToolchainConfigurationState::Draft
        } else {
            LanguageToolchainConfigurationState::Configured
        }
    }

    /// Clear the configured TypeScript toolchain without deleting its cache
    /// or changing editor buffers and dirty state.
    pub fn clear_typescript_toolchain(&mut self) {
        let selected_server_is_typescript = self
            .lsp_session
            .selected_server_id()
            .is_some_and(|server_id| matches!(server_id.0, 102 | 106 | 107 | 108));
        if selected_server_is_typescript {
            self.terminalize_pending_lsp_writes(
                None,
                LanguageToolingStatusKind::Cancelled,
                "TypeScript language server configuration was cleared",
            );
            self.lsp_session.reset_to_idle();
        }
        let prior_node = self.typescript_node_approval.take();
        let typescript_server_ids = [
            LanguageServerId(102),
            LanguageServerId(106),
            LanguageServerId(107),
            LanguageServerId(108),
        ];
        let mut removed_nodes = HashSet::new();
        if let Some(node) = prior_node.clone() {
            removed_nodes.insert(node);
        }
        for server_id in typescript_server_ids {
            if let Some(path) = self.language_server_configured_paths.get(&server_id) {
                removed_nodes.insert(path.clone());
            }
            if let Some(config) = self.language_server_local_downloads.get(&server_id) {
                removed_nodes.insert(config.node_path.clone());
            }
            if let Some(config) = self.language_server_downloaded.get(&server_id) {
                removed_nodes.insert(config.approved_node.canonical_path().to_path_buf());
            }
            if let Some(config) = self.typescript_bundles.get(&server_id) {
                removed_nodes.insert(config.node_path.clone());
            }
            self.typescript_bundles.remove(&server_id);
            self.language_server_configured_paths.remove(&server_id);
            self.language_server_local_downloads.remove(&server_id);
            self.language_server_downloaded.remove(&server_id);
        }
        self.language_toolchain_settings.typescript = None;
        for node in removed_nodes {
            let retained_elsewhere = self
                .language_server_configured_paths
                .values()
                .any(|path| path == &node)
                || self
                    .language_server_local_downloads
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path())
                || self
                    .language_server_downloaded
                    .values()
                    .any(|config| config.approved_node.canonical_path() == node.as_path())
                || self
                    .typescript_bundles
                    .values()
                    .any(|config| config.node_path.as_path() == node.as_path());
            if !retained_elsewhere {
                let _ = self.language_startup_authority.revoke_exact_binary(&node);
            }
        }
    }

    /// Configure the operator-selected local Python toolchain for the active
    /// trusted workspace.
    ///
    /// Both arguments must be explicit paths. A bare executable name is
    /// refused before any filesystem, policy-store, or process effect: this
    /// method never performs `PATH` discovery, and there is no fallback that
    /// reintroduces it.
    ///
    /// Like the TypeScript path, this records canonical input metadata and one
    /// exact-binary allowance per executable. It is not a capability decision,
    /// not a receipt, and not permission to spawn anything; nothing here starts
    /// an interpreter, a formatter, or a language server.
    pub fn configure_python_toolchain(
        &mut self,
        interpreter_executable: impl AsRef<Path>,
        formatter_executable: impl AsRef<Path>,
    ) -> Result<(), AppCompositionError> {
        if self.active_documents.workspace_id().is_none() {
            return Err(AppCompositionError::WorkspaceNotOpen);
        }
        if self.active_documents.active_workspace_trust != Some(WorkspaceTrustState::Trusted) {
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_workspace_untrusted".to_string(),
                message: "Python toolchains require a trusted workspace".to_string(),
            }));
        }

        // Validate every input before changing the policy store or replacing
        // the existing configuration, so an invalid second path is atomic.
        let interpreter = explicit_canonical_executable_path(
            interpreter_executable.as_ref(),
            "Python interpreter",
        )?;
        let formatter =
            explicit_canonical_executable_path(formatter_executable.as_ref(), "Python formatter")?;

        let previous = self.language_toolchain_settings.python.clone();
        let interpreter_was_allowed = self.exact_binary_retained_elsewhere(&interpreter);
        self.language_startup_authority
            .allow_exact_binary(&interpreter)
            .map_err(|error| {
                AppCompositionError::Protocol(ProtocolError {
                    code: "language_toolchain_python_interpreter_invalid".to_string(),
                    message: error.to_string(),
                })
            })?;
        if let Err(error) = self
            .language_startup_authority
            .allow_exact_binary(&formatter)
        {
            // The interpreter allowance is only rolled back when this call
            // introduced it; a grant some other configuration already owned
            // survives a failed formatter grant untouched.
            if !interpreter_was_allowed {
                let _ = self
                    .language_startup_authority
                    .revoke_exact_binary(&interpreter);
            }
            return Err(AppCompositionError::Protocol(ProtocolError {
                code: "language_toolchain_python_formatter_invalid".to_string(),
                message: error.to_string(),
            }));
        }

        self.language_toolchain_settings.schema_version = 1;
        self.language_toolchain_settings.python = Some(PythonToolchainSettings {
            interpreter_executable: CanonicalPath(
                interpreter
                    .to_str()
                    .expect("canonical interpreter path is UTF-8")
                    .to_string(),
            ),
            formatter_executable: CanonicalPath(
                formatter
                    .to_str()
                    .expect("canonical formatter path is UTF-8")
                    .to_string(),
            ),
        });

        if let Some(previous) = previous {
            self.revoke_unshared_exact_binaries(&previous);
        }
        Ok(())
    }

    /// Clear the configured Python toolchain without deleting anything on disk
    /// and without changing editor buffers or dirty state.
    ///
    /// Each replaced executable's allowance is revoked only when no other
    /// configuration still holds that exact binary.
    pub fn clear_python_toolchain(&mut self) {
        let Some(previous) = self.language_toolchain_settings.python.take() else {
            return;
        };
        self.revoke_unshared_exact_binaries(&previous);
    }

    /// Return the Pyright configuration payload built from the configured
    /// Python toolchain, or `None` when no Python toolchain is configured.
    ///
    /// This is the reachable entry point a launch path reads;
    /// [`pyright_initialization_options`] is the pure builder behind it. This
    /// method only reads already recorded settings: it starts no process,
    /// grants no allowance, and performs no `PATH` lookup, so calling it is not
    /// a capability decision.
    pub fn pyright_configuration_payload(&self) -> Option<serde_json::Value> {
        let python = self.language_toolchain_settings.python.as_ref()?;
        Some(pyright_initialization_options(python))
    }

    /// Revoke the allowances of a replaced Python configuration, skipping any
    /// exact binary that a still-live configuration continues to hold.
    ///
    /// Call this only after `language_toolchain_settings.python` already holds
    /// the replacement (or `None`), so the outgoing record cannot count as its
    /// own retainer.
    fn revoke_unshared_exact_binaries(&mut self, previous: &PythonToolchainSettings) {
        for value in [
            &previous.interpreter_executable,
            &previous.formatter_executable,
        ] {
            let path = PathBuf::from(&value.0);
            if !self.exact_binary_retained_elsewhere(&path) {
                let _ = self.language_startup_authority.revoke_exact_binary(&path);
            }
        }
    }

    /// Whether any live language configuration still holds this exact binary.
    fn exact_binary_retained_elsewhere(&self, path: &Path) -> bool {
        let canonical = path.to_str().map(|value| CanonicalPath(value.to_string()));
        if let Some(python) = self.language_toolchain_settings.python.as_ref()
            && (canonical.as_ref() == Some(&python.interpreter_executable)
                || canonical.as_ref() == Some(&python.formatter_executable))
        {
            return true;
        }
        if let Some(typescript) = self.language_toolchain_settings.typescript.as_ref()
            && canonical.as_ref() == Some(&typescript.node_executable)
        {
            return true;
        }
        if self.typescript_node_approval.as_deref() == Some(path) {
            return true;
        }
        self.language_server_configured_paths
            .values()
            .any(|value| value.as_path() == path)
            || self
                .language_server_local_downloads
                .values()
                .any(|config| config.node_path.as_path() == path)
            || self
                .language_server_downloaded
                .values()
                .any(|config| config.approved_node.canonical_path() == path)
            || self
                .typescript_bundles
                .values()
                .any(|config| config.node_path.as_path() == path)
    }
}

/// Canonicalize one explicitly selected executable path.
///
/// A bare name with no directory component is refused first, with a distinct
/// code, before `canonicalize` is called: `python` and `black` are exactly the
/// inputs a `PATH` lookup would consume, and resolving them relative to the
/// process working directory would be the same defect wearing a different hat.
fn explicit_canonical_executable_path(
    path: &Path,
    label: &str,
) -> Result<PathBuf, AppCompositionError> {
    let bare_name = match path.parent() {
        None => true,
        Some(parent) => parent.as_os_str().is_empty(),
    };
    if bare_name {
        let shown = path.display();
        return Err(invalid_toolchain_input(
            "language_toolchain_path_not_explicit",
            format!("{label} is the bare name \"{shown}\"; give an explicit path"),
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        let shown = path.display();
        invalid_toolchain_input(
            "language_toolchain_input_invalid",
            format!("{label} \"{shown}\" is invalid: {error}"),
        )
    })?;
    if !canonical.is_file() {
        let shown = canonical.display();
        return Err(invalid_toolchain_input(
            "language_toolchain_input_invalid",
            format!("{label} \"{shown}\" must be a regular file"),
        ));
    }
    if canonical.to_str().is_none() {
        return Err(invalid_toolchain_input(
            "language_toolchain_input_invalid",
            format!("{label} path is not valid UTF-8"),
        ));
    }
    Ok(canonical)
}

/// Build one metadata-only refusal for a rejected toolchain input.
fn invalid_toolchain_input(code: &str, message: String) -> AppCompositionError {
    AppCompositionError::Protocol(ProtocolError {
        code: code.to_string(),
        message,
    })
}

/// Build the Pyright `initializationOptions` object for a configured Python
/// toolchain.
///
/// Pure: no filesystem, no process, no policy store. See the module docs for
/// where each key comes from in the pinned Pyright 1.1.400 bundle and for why
/// `pythonPath` is carried under `settings` rather than at the top level.
pub fn pyright_initialization_options(python: &PythonToolchainSettings) -> serde_json::Value {
    serde_json::json!({
        "diagnosticMode": "openFilesOnly",
        "disablePullDiagnostics": false,
        "settings": {
            "python": {
                "pythonPath": python.interpreter_executable.0.as_str()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyright_initialization_options_carry_the_canonical_interpreter_path() {
        let interpreter = if cfg!(windows) {
            "C:\\python\\3.12\\python.exe"
        } else {
            "/opt/python/3.12/bin/python3.12"
        };
        let formatter = if cfg!(windows) {
            "C:\\python\\3.12\\Scripts\\black.exe"
        } else {
            "/opt/python/3.12/bin/black"
        };
        let python = PythonToolchainSettings {
            interpreter_executable: CanonicalPath(interpreter.to_string()),
            formatter_executable: CanonicalPath(formatter.to_string()),
        };

        let options = pyright_initialization_options(&python);
        let expected = serde_json::json!({
            "diagnosticMode": "openFilesOnly",
            "disablePullDiagnostics": false,
            "settings": {
                "python": {
                    "pythonPath": interpreter
                }
            }
        });
        assert_eq!(
            options, expected,
            "the payload must be exactly the keys Pyright 1.1.400 reads, and the \
             interpreter path must be the absolute canonical one"
        );

        let python_path = options["settings"]["python"]["pythonPath"]
            .as_str()
            .expect("pythonPath is a string");
        assert!(
            Path::new(python_path).is_absolute(),
            "a relative or bare pythonPath would send Pyright back to PATH"
        );
        // Pyright's `isPythonBinary` discards exactly these two values and
        // then spawns `python` itself, which is the recorded defect. A
        // canonical path is never one of them; the assertions pin the reason
        // the payload is shaped this way.
        assert_ne!(python_path, "python");
        assert_ne!(python_path, "python3");
        // The formatter is deliberately absent: Pyright does not run one, and
        // no key for it was found in the pinned bundle.
        assert!(options["settings"]["python"].get("formatterPath").is_none());
    }
}
