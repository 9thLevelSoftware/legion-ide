//! Construction helpers for [`AppCommandOutcome`].
//!
//! A module of its own because `lib.rs` is a chokepoint with a growth
//! budget, and an impl block is exactly the kind of thing that belongs
//! outside it.

use crate::AppCommandOutcome;
use legion_protocol::LanguageToolingProjection;
impl AppCommandOutcome {
    /// Wrap a language-tooling projection as an outcome.
    ///
    /// The projection is by far the largest thing `AppCommandOutcome` carries,
    /// and the enum is returned by value from every command — so a vim mode
    /// change and a one-word string were already paying for it. Two
    /// call-hierarchy fields pushed it past clippy's `large_enum_variant`
    /// threshold, which is the lint noticing a cost that was always there.
    ///
    /// A constructor rather than `Box::new` at each of the fifteen call sites,
    /// where wrapping a multi-line expression ending in `?` is easy to get
    /// subtly wrong.
    pub fn language_tooling(projection: LanguageToolingProjection) -> Self {
        Self::LanguageToolingUpdated(Box::new(projection))
    }
}
