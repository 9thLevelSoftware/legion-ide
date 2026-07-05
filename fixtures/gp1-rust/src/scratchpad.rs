// GP-1 smoke fixture: scratchpad module.
//
// This file is edited at runtime by the GP-1 smoke (step s3) to introduce and
// then fix a compile error. At rest it contains only valid Rust code so that
// `cargo test` in the fixture passes without intervention.

/// No-op placeholder function. The smoke replaces this entire file at runtime.
#[allow(dead_code)]
pub fn scratchpad() {}
