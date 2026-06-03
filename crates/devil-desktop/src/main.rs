//! Legion IDE renderer-backed desktop entry point.

use anyhow::Result;

fn main() -> Result<()> {
    devil_desktop::workflow::run_from_env()
}
