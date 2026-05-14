//! Devil IDE desktop application entry point.

use std::env;
use std::io::{self, Write};

use anyhow::Result;
use devil_app::{AppComposition, AppSaveOutcome};
use devil_protocol::{PrincipalId, WorkspaceTrustState};

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "scratch.txt".to_string());

    let root = env::current_dir()?;
    let mut app = AppComposition::new();
    app.open_workspace(
        root,
        WorkspaceTrustState::Trusted,
        PrincipalId("cli".to_string()),
    )?;
    let file_id = app.open_file(path)?;

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
