//! Unit tests for the stale-snapshot rejection gate (WS-LANG-01 LANG.07).
//!
//! Verifies that `is_stale_response` correctly identifies stale vs. fresh
//! responses based on snapshot identity comparison.

use legion_app::language::is_stale_response;
use legion_protocol::SnapshotId;

#[test]
fn response_for_older_snapshot_is_stale() {
    // A response issued against snapshot 1 is stale when the buffer is now at snapshot 2.
    assert!(is_stale_response(SnapshotId(1), SnapshotId(2)));
}

#[test]
fn response_for_current_snapshot_is_fresh() {
    // A response issued against snapshot 2 is fresh when the buffer is still at snapshot 2.
    assert!(!is_stale_response(SnapshotId(2), SnapshotId(2)));
}

#[test]
fn response_for_future_snapshot_is_stale() {
    // A response issued against snapshot 5 while the buffer is at snapshot 3 is stale.
    // (Defensive: issued > current is also stale — snapshots must match exactly.)
    assert!(is_stale_response(SnapshotId(5), SnapshotId(3)));
}
