use legion_protocol::WorkbenchTelemetryConsent;
use legion_ui::SettingsProjection;

/// Build the telemetry consent payload used by the first-run crash-report choice.
pub fn crash_reporting_consent(enabled: bool) -> WorkbenchTelemetryConsent {
    WorkbenchTelemetryConsent {
        enabled,
        crash_reports_enabled: enabled,
        raw_source_allowed: false,
        consent_label: if enabled {
            "crash-reports".to_string()
        } else {
            "local-only".to_string()
        },
        schema_version: 1,
    }
}

/// Apply the first-run crash-report choice to the app-owned settings projection.
pub fn apply_crash_reporting_consent(settings: &mut SettingsProjection, enabled: bool) {
    settings.telemetry = crash_reporting_consent(enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_reporting_consent_is_opt_in_when_enabled() {
        let consent = crash_reporting_consent(true);

        assert!(consent.enabled);
        assert!(consent.crash_reports_enabled);
        assert!(!consent.raw_source_allowed);
        assert_eq!(consent.consent_label, "crash-reports");
        assert_eq!(consent.schema_version, 1);
    }

    #[test]
    fn crash_reporting_consent_defaults_to_local_only_when_disabled() {
        let consent = crash_reporting_consent(false);

        assert!(!consent.enabled);
        assert!(!consent.crash_reports_enabled);
        assert!(!consent.raw_source_allowed);
        assert_eq!(consent.consent_label, "local-only");
        assert_eq!(consent.schema_version, 1);
    }

    #[test]
    fn apply_crash_reporting_consent_updates_settings_projection() {
        let mut settings = SettingsProjection::default();

        apply_crash_reporting_consent(&mut settings, true);

        assert!(settings.telemetry.crash_reports_enabled);
        assert_eq!(settings.telemetry.consent_label, "crash-reports");
    }
}
