//! Deterministic packaging metadata for the desktop adapter.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The Windows executable produced by the `legion-desktop` package.
pub const WINDOWS_EXECUTABLE_NAME: &str = "legion-desktop.exe";

/// The manifest file written into dry-run or package output directories.
pub const PACKAGE_MANIFEST_NAME: &str = "legion-desktop-package-manifest.txt";

/// Supported package build profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageProfile {
    /// Use the debug cargo target directory.
    Debug,
    /// Use the release cargo target directory.
    Release,
}

impl PackageProfile {
    /// Returns the cargo profile label used in target paths.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    /// Returns cargo arguments for the selected profile.
    #[must_use]
    pub fn cargo_args(self) -> Vec<&'static str> {
        let mut args = vec!["build", "-p", "legion-desktop"];
        if matches!(self, Self::Release) {
            args.push("--release");
        }
        args
    }
}

/// Inputs used to compute a package layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPackageConfig {
    /// Repository root containing `Cargo.toml`.
    pub workspace_root: PathBuf,
    /// Output directory for the package bundle.
    pub output_dir: PathBuf,
    /// Cargo build profile.
    pub profile: PackageProfile,
    /// Optional explicit cargo target triple to cross-compile the Windows
    /// executable. Required when packaging from a non-Windows host.
    pub target_triple: Option<String>,
}

impl WindowsPackageConfig {
    /// Creates a new Windows package configuration.
    #[must_use]
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        profile: PackageProfile,
    ) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            output_dir: output_dir.into(),
            profile,
            target_triple: None,
        }
    }

    /// Sets the cargo target triple used to cross-compile the Windows binary.
    #[must_use]
    pub fn with_target_triple(mut self, target_triple: impl Into<String>) -> Self {
        self.target_triple = Some(target_triple.into());
        self
    }
}

/// Resolved package layout and commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPackagePlan {
    /// Cargo build profile.
    pub profile: PackageProfile,
    /// Cargo command arguments needed before copying the executable.
    pub cargo_args: Vec<String>,
    /// Expected built executable path.
    pub executable_source: PathBuf,
    /// Package output directory.
    pub package_dir: PathBuf,
    /// Final executable path inside the package output directory.
    pub executable_destination: PathBuf,
    /// Manifest path inside the package output directory.
    pub manifest_path: PathBuf,
}

/// Packaging plan errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackagePlanError {
    /// The workspace root does not look like the repository root.
    #[error("workspace root `{0}` does not contain Cargo.toml")]
    MissingWorkspaceManifest(String),
    /// The output directory is not named.
    #[error("output directory must not be empty")]
    EmptyOutputDirectory,
    /// Packaging from a non-Windows host requires an explicit Windows target triple.
    #[error(
        "packaging a Windows executable from a non-Windows host requires an explicit target triple"
    )]
    MissingWindowsTargetTriple,
    /// The configured target triple was empty.
    #[error("target triple must not be empty")]
    EmptyTargetTriple,
}

/// Builds a deterministic Windows package plan without touching the filesystem.
///
/// This intentionally does not create an installer. Phase 6 packages only the
/// current desktop executable and metadata needed to prove repeatable dry runs.
pub fn plan_windows_package(
    config: &WindowsPackageConfig,
) -> Result<WindowsPackagePlan, PackagePlanError> {
    if config.output_dir.as_os_str().is_empty() {
        return Err(PackagePlanError::EmptyOutputDirectory);
    }

    if !config.workspace_root.join("Cargo.toml").is_file() {
        return Err(PackagePlanError::MissingWorkspaceManifest(
            config.workspace_root.display().to_string(),
        ));
    }

    // Resolve the effective target triple. A non-Windows host cannot produce a
    // Windows executable from the host target, so an explicit triple is
    // required to avoid pointing the plan at a host binary with no `.exe`.
    let target_triple = match &config.target_triple {
        Some(triple) => {
            if triple.trim().is_empty() {
                return Err(PackagePlanError::EmptyTargetTriple);
            }
            Some(triple.as_str())
        }
        None => {
            if !cfg!(target_os = "windows") {
                return Err(PackagePlanError::MissingWindowsTargetTriple);
            }
            None
        }
    };

    let mut target_dir = config.workspace_root.join("target");
    if let Some(triple) = target_triple {
        target_dir = target_dir.join(triple);
    }
    let executable_source = target_dir
        .join(config.profile.as_str())
        .join(WINDOWS_EXECUTABLE_NAME);
    let executable_destination = config.output_dir.join(WINDOWS_EXECUTABLE_NAME);
    let manifest_path = config.output_dir.join(PACKAGE_MANIFEST_NAME);

    let mut cargo_args: Vec<String> = config
        .profile
        .cargo_args()
        .into_iter()
        .map(str::to_string)
        .collect();
    if let Some(triple) = target_triple {
        cargo_args.push("--target".to_string());
        cargo_args.push(triple.to_string());
    }

    Ok(WindowsPackagePlan {
        profile: config.profile,
        cargo_args,
        executable_source,
        package_dir: config.output_dir.clone(),
        executable_destination,
        manifest_path,
    })
}

/// Formats a package manifest for dry-run or real package output.
#[must_use]
pub fn package_manifest(plan: &WindowsPackagePlan, dry_run: bool) -> String {
    let cargo_command = std::iter::once("cargo".to_string())
        .chain(plan.cargo_args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        "package: legion-desktop\r\nplatform: windows\r\nprofile: {}\r\ndry_run: {}\r\ncargo_command: {}\r\nsource_executable: {}\r\npackage_directory: {}\r\npackage_executable: {}\r\n",
        plan.profile.as_str(),
        dry_run,
        cargo_command,
        display_path(&plan.executable_source),
        display_path(&plan.package_dir),
        display_path(&plan.executable_destination),
    )
}

/// Writes the package manifest to the plan output directory.
pub fn write_package_manifest(plan: &WindowsPackagePlan, dry_run: bool) -> io::Result<()> {
    fs::create_dir_all(&plan.package_dir)?;
    fs::write(&plan.manifest_path, package_manifest(plan, dry_run))
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
