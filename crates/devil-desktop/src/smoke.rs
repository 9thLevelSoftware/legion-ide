//! Renderer smoke harness boundary.

/// Runs an inert smoke harness placeholder from adapter-owned arguments.
pub fn run_smoke_from_args<I, S>(_args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    Ok(())
}
