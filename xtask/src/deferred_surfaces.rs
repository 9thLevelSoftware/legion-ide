//! The deferred-surface gate: a frozen surface cannot thaw by editing one cell.
//!
//! ADR-0046 freezes surface expansion until PR-UI-001 reaches "product workflow
//! validated", and keeps three gates deferred by name: PR-VSC-002 (isolated
//! extension host), PR-ENT-001 (remote development UX), and PR-ENT-002
//! (collaboration and admin controls). Roadmap task P9.F3.T4 states the rule
//! that keeps them honest: **each surface needs its own ADR, policy, tests, and
//! product evidence before its readiness status changes.**
//!
//! Nothing enforced that. The readiness ledger is a markdown table, and a
//! surface could be promoted from "Deferred" to "Substrate validated" — or
//! further — by editing a single cell in a documentation file. The four
//! artifacts the rule demands would still be missing, and the row would read as
//! though they were not.
//!
//! This gate makes the promotion cost what the rule says it costs. A deferred
//! row may stay deferred with no artifacts at all. The moment it claims
//! anything else, all four must exist and be named, and the gate fails with the
//! specific missing one rather than a general complaint.
//!
//! ## Why a text gate rather than something cleverer
//!
//! The same reasoning as `intent_reachability`: a gate whose verdict nobody can
//! predict is a gate people route around. "The row says X and these four files
//! exist" is coarse, but it is exactly the property the rule asks for, and it
//! cannot be satisfied by accident.

use std::path::Path;

use serde::Deserialize;

/// Configuration: which surfaces are frozen, and what unfreezing requires.
#[derive(Debug, Clone, Deserialize)]
pub struct DeferredSurfacesConfig {
    /// Readiness ledger to read.
    pub ledger: String,
    /// Status strings that count as "still deferred", requiring no artifacts.
    pub deferred_statuses: Vec<String>,
    /// Ledger row whose promotion lifts the ADR-0046 freeze.
    pub freeze_gate: String,
    /// Status `freeze_gate` must reach before any frozen surface may move.
    pub freeze_lifted_status: String,
    /// The frozen surfaces.
    pub surfaces: Vec<DeferredSurface>,
}

/// One frozen surface and the artifacts its promotion requires.
#[derive(Debug, Clone, Deserialize)]
pub struct DeferredSurface {
    /// Ledger row id, e.g. `PR-ENT-001`.
    pub gate: String,
    /// Why this surface is frozen. Required, and read by people.
    pub reason: String,
    /// Path to the ADR that must exist before promotion.
    pub adr: String,
    /// Path to the policy that must exist before promotion.
    pub policy: String,
    /// Path to the test target that must exist before promotion.
    pub tests: String,
    /// Path to the product evidence that must exist before promotion.
    pub evidence: String,
}

/// A surface that claims more readiness than its artifacts support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedPromotion {
    /// Ledger row id.
    pub gate: String,
    /// The status the row currently claims.
    pub status: String,
    /// Which required artifacts are absent.
    pub missing: Vec<String>,
}

/// Why the gate could not reach a verdict.
#[derive(Debug)]
pub enum GateError {
    /// The ledger could not be read.
    Ledger(String),
    /// A configured surface has no row in the ledger.
    RowMissing(Vec<String>),
    /// A surface entry carries no reason.
    ReasonMissing(String),
}

impl DeferredSurfacesConfig {
    /// Read the config from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let body = std::fs::read_to_string(path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        toml::from_str(&body).map_err(|err| format!("cannot parse {}: {err}", path.display()))
    }
}

/// Check every frozen surface against the ledger.
///
/// `Ok(Ok(count))` means all `count` surfaces are either still deferred or
/// carry every artifact their promotion requires.
pub fn run_deferred_surfaces(
    workspace_root: &Path,
    config: &DeferredSurfacesConfig,
) -> Result<Result<usize, Vec<UnsupportedPromotion>>, GateError> {
    for surface in &config.surfaces {
        if surface.reason.trim().is_empty() {
            return Err(GateError::ReasonMissing(surface.gate.clone()));
        }
    }

    let ledger = std::fs::read_to_string(workspace_root.join(&config.ledger))
        .map_err(|err| GateError::Ledger(format!("{}: {err}", config.ledger)))?;

    // The freeze itself is the first condition, and the one that bites today.
    //
    // Every one of these surfaces already has its own ADR — remote has four —
    // so an artifacts-only check would wave through a promotion that ADR-0046
    // clause 2 forbids outright. The artifacts are necessary; the freeze being
    // lifted is what makes them sufficient.
    let freeze_lifted = row_status(&ledger, &config.freeze_gate)
        .ok_or_else(|| GateError::RowMissing(vec![config.freeze_gate.clone()]))?
        .starts_with(config.freeze_lifted_status.as_str());

    let mut absent_rows = Vec::new();
    let mut unsupported = Vec::new();
    for surface in &config.surfaces {
        let Some(status) = row_status(&ledger, &surface.gate) else {
            absent_rows.push(surface.gate.clone());
            continue;
        };
        if config
            .deferred_statuses
            .iter()
            .any(|deferred| status.starts_with(deferred.as_str()))
        {
            continue;
        }
        if !freeze_lifted {
            unsupported.push(UnsupportedPromotion {
                gate: surface.gate.clone(),
                status,
                missing: vec![format!(
                    "the ADR-0046 freeze is still in force: {} is not \"{}\"",
                    config.freeze_gate, config.freeze_lifted_status
                )],
            });
            continue;
        }
        let missing: Vec<String> = [
            ("ADR", &surface.adr),
            ("policy", &surface.policy),
            ("tests", &surface.tests),
            ("evidence", &surface.evidence),
        ]
        .into_iter()
        .filter(|(_, path)| !workspace_root.join(path.as_str()).exists())
        .map(|(label, path)| format!("{label} ({path})"))
        .collect();
        if !missing.is_empty() {
            unsupported.push(UnsupportedPromotion {
                gate: surface.gate.clone(),
                status,
                missing,
            });
        }
    }

    // A configured surface with no ledger row is an error rather than a pass:
    // deleting the row would otherwise be a way to escape the gate entirely,
    // which is a louder version of the edit it exists to prevent.
    if !absent_rows.is_empty() {
        return Err(GateError::RowMissing(absent_rows));
    }

    if unsupported.is_empty() {
        Ok(Ok(config.surfaces.len()))
    } else {
        Ok(Err(unsupported))
    }
}

/// The status cell of the ledger row naming `gate`.
///
/// The readiness matrix is a pipe table whose columns are track, gate,
/// acceptance, status, evidence. Reading the fourth cell rather than searching
/// the whole line matters: the acceptance and evidence cells routinely contain
/// the words "validated" and "deferred" in prose, and a line-wide search would
/// read those as the row's status.
fn row_status(ledger: &str, gate: &str) -> Option<String> {
    for line in ledger.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed.trim_matches('|').split('|').collect();
        if cells.len() < 4 {
            continue;
        }
        if !cells[1].trim().starts_with(gate) {
            continue;
        }
        return Some(cells[3].trim().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = "\
| Track | Gate | Acceptance Criteria | Current Status | Current Evidence |
| --- | --- | --- | --- | --- |
| VS Code | PR-VSC-002 isolated extension host | Runtime execution is deferred and validated later. | Deferred with explicit cut lines | none |
| Remote | PR-ENT-001 remote development UX | Reconnect is a product workflow. | Product workflow validated | some evidence |
";

    #[test]
    fn the_status_cell_is_read_not_the_whole_line() {
        // The acceptance cell for PR-VSC-002 contains the word "validated" and
        // the row is deferred. A line-wide search would promote it by accident,
        // which is the opposite of what this gate is for.
        assert_eq!(
            row_status(LEDGER, "PR-VSC-002").as_deref(),
            Some("Deferred with explicit cut lines")
        );
    }

    #[test]
    fn a_promoted_row_reports_its_status() {
        assert_eq!(
            row_status(LEDGER, "PR-ENT-001").as_deref(),
            Some("Product workflow validated")
        );
    }

    #[test]
    fn an_absent_row_is_none_rather_than_an_empty_status() {
        // Returning an empty status here would let a deleted row read as
        // "deferred" and pass silently — escaping the gate by removing the
        // thing it checks.
        assert!(row_status(LEDGER, "PR-ENT-999").is_none());
    }
}
