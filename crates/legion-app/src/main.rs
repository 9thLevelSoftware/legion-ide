//! Legion IDE desktop application entry point.

use std::env;
use std::io::{self, Write};

use anyhow::Result;
use legion_app::{AppComposition, AppSaveOutcome};
use legion_protocol::{PrincipalId, WorkspaceTrustState};

fn main() -> Result<()> {
    let explicit_path = env::args().nth(1);
    let path = explicit_path
        .clone()
        .unwrap_or_else(|| "scratch.txt".to_string());

    let root = env::current_dir()?;
    let mut app = AppComposition::new();
    match app.reap_orphaned_delegated_task_sandboxes() {
        Ok(removed) if !removed.is_empty() => {
            eprintln!(
                "Reaped {} orphaned delegated-task sandbox(es):",
                removed.len()
            );
            for path in &removed {
                eprintln!("  {}", path.display());
            }
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!("Warning: failed to reap orphaned delegated-task sandboxes: {err}");
        }
    }
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("cli".to_string()),
    )?;
    // When no path was supplied we fall back to the scratch buffer. If that
    // default file does not exist yet, create it rather than aborting startup
    // (the Existing open intent reads metadata and fails on a missing file).
    let file_id = if explicit_path.is_none() && !std::path::Path::new(&path).exists() {
        app.open_new_file(&path)?
    } else {
        app.open_file(&path)?
    };

    println!("Opened file id {:?}", file_id);
    println!("Commands: :w | :q");

    let mut input = String::new();
    loop {
        print!("> ");
        io::stdout().flush()?;

        input.clear();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        match input.trim_end() {
            ":q" => break,
            ":w" => match app.save_active_buffer()? {
                AppSaveOutcome::Saved(save) => {
                    println!(
                        "Saved file_id={:?} snapshot={} hash={}",
                        save.file_id, save.snapshot_id.0, save.content_hash
                    );
                }
                AppSaveOutcome::Rejected(response) => {
                    println!("Save did not apply: {response:?}");
                }
            },
            _ => {}
        }
    }

    Ok(())
}
