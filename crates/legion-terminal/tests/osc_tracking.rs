use legion_terminal::osc::{TerminalShellBoundary, parse_terminal_shell_output};
use legion_terminal::session::TerminalSessionMetadata;

#[test]
fn osc_parser_keeps_unterminated_sequences_visible() {
    let payload = "before\x1b]7;file://localhost/home";

    let parsed = parse_terminal_shell_output(payload);

    assert_eq!(parsed.visible_output, "before\x1b]7;file://localhost/home");
    assert_eq!(parsed.cwd, None);
    assert_eq!(parsed.exit_code, None);
    assert_eq!(parsed.boundary, None);
}

#[test]
fn osc7_cwd_decodes_localhost_unc_windows_and_percent_paths() {
    let local = parse_terminal_shell_output("\x1b]7;file://localhost/home/user\x07");
    assert_eq!(local.cwd.as_deref(), Some("/home/user"));

    let windows = parse_terminal_shell_output("\x1b]7;file:///C:/Users/My%20Project\x1b\\");
    assert_eq!(windows.cwd.as_deref(), Some("C:/Users/My Project"));

    let unc = parse_terminal_shell_output("\x1b]7;file://server/share/dir\x1b\\");
    assert_eq!(unc.cwd.as_deref(), Some("//server/share/dir"));
}

#[test]
fn osc133_tracks_boundary_and_exit_code_metadata() {
    let parsed = parse_terminal_shell_output("\x1b]133;B\x1b\\run\x1b]133;D;42\x1b\\");

    assert_eq!(parsed.visible_output, "run");
    assert_eq!(
        parsed.boundary,
        Some(TerminalShellBoundary::CommandFinished)
    );
    assert_eq!(parsed.exit_code, Some(42));
}

#[test]
fn terminal_session_metadata_merges_latest_osc_projection() {
    let mut metadata = TerminalSessionMetadata::default();
    let cwd_projection = parse_terminal_shell_output("\x1b]7;file:///repo\x1b\\");
    metadata.apply_shell_projection(&cwd_projection);
    assert_eq!(metadata.cwd.as_deref(), Some("/repo"));
    assert_eq!(metadata.exit_code, None);

    let exit_projection = parse_terminal_shell_output("\x1b]133;D;9\x1b\\");
    metadata.apply_shell_projection(&exit_projection);
    assert_eq!(metadata.cwd.as_deref(), Some("/repo"));
    assert_eq!(metadata.exit_code, Some(9));
    assert_eq!(
        metadata.boundary,
        Some(TerminalShellBoundary::CommandFinished)
    );
}
