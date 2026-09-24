//! Shared deadline arbitration for one server-originated `workspace/applyEdit`.
//!
//! This module is intentionally independent of the app and LSP wiring.  The
//! callback and proposal application path can share an [`ApplyEditDecision`]
//! so a timeout cannot manufacture a reply after the apply path has claimed
//! the operation.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

const PENDING: u8 = 0;
const CLAIMED: u8 = 1;
const FINISHED: u8 = 2;
const EXPIRED: u8 = 3;

/// The terminal result shared by the callback and apply path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyEditDecisionResult {
    pub(crate) applied: bool,
    pub(crate) failure_reason: Option<String>,
}

impl ApplyEditDecisionResult {
    pub(crate) fn applied() -> Self {
        Self {
            applied: true,
            failure_reason: None,
        }
    }

    pub(crate) fn rejected(reason: impl Into<String>) -> Self {
        Self {
            applied: false,
            failure_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug)]
struct Shared {
    state: AtomicU8,
    result: Mutex<Option<ApplyEditDecisionResult>>,
    completed: Condvar,
}

/// Bounded, one-shot arbitration for an apply-edit request.
#[derive(Debug, Clone)]
pub struct ApplyEditDecision {
    shared: Arc<Shared>,
}

/// Proof that this decision won `Pending -> Claimed`.
#[derive(Debug)]
pub(crate) struct ApplyEditClaim {
    shared: Arc<Shared>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaimError {
    Expired,
    AlreadyClaimed,
    AlreadyFinished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeadlineDecision {
    /// The timeout won `Pending -> Expired`; the caller must return false.
    Expired,
    /// The apply path claimed the operation; wait for its actual result.
    Claimed,
    /// Completion was already published, including a completion queued at the
    /// deadline boundary.
    Finished(ApplyEditDecisionResult),
}

impl Default for ApplyEditDecision {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplyEditDecision {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: AtomicU8::new(PENDING),
                result: Mutex::new(None),
                completed: Condvar::new(),
            }),
        }
    }

    /// Atomically claims the decision, provided the deadline has not elapsed.
    pub(crate) fn claim(&self, deadline: Instant) -> Result<ApplyEditClaim, ClaimError> {
        if Instant::now() >= deadline {
            let _ = self.expire_pending();
        }
        match self.shared.state.compare_exchange(
            PENDING,
            CLAIMED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(ApplyEditClaim {
                shared: Arc::clone(&self.shared),
            }),
            Err(PENDING) => Err(ClaimError::Expired),
            Err(CLAIMED) => Err(ClaimError::AlreadyClaimed),
            Err(FINISHED) => Err(ClaimError::AlreadyFinished),
            Err(EXPIRED) => Err(ClaimError::Expired),
            Err(_) => unreachable!("apply-edit decision state is bounded"),
        }
    }

    /// Resolves the callback's deadline race. A claimed operation is never
    /// converted into a timeout; the caller must await its actual result.
    pub(crate) fn on_deadline(&self) -> DeadlineDecision {
        if self.expire_pending() {
            return DeadlineDecision::Expired;
        }
        match self.shared.state.load(Ordering::Acquire) {
            CLAIMED => DeadlineDecision::Claimed,
            FINISHED => DeadlineDecision::Finished(self.result()),
            EXPIRED => DeadlineDecision::Expired,
            PENDING => unreachable!("pending state must be expired or claimed"),
            _ => unreachable!("apply-edit decision state is bounded"),
        }
    }

    fn expire_pending(&self) -> bool {
        self.shared
            .state
            .compare_exchange(PENDING, EXPIRED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn result(&self) -> ApplyEditDecisionResult {
        self.shared
            .result
            .lock()
            .expect("apply-edit decision result lock")
            .clone()
            .expect("finished apply-edit decision must contain a result")
    }

    /// Waits for a claimed operation to finish. This intentionally has no
    /// deadline: once claim wins, returning false at the old deadline would
    /// make the wire result disagree with the committed mutation.
    pub(crate) fn wait_for_result(&self) -> ApplyEditDecisionResult {
        let mut result = self.shared.result.lock().expect("apply-edit decision lock");
        while result.is_none() {
            result = self
                .shared
                .completed
                .wait(result)
                .expect("apply-edit decision lock");
        }
        result.clone().expect("result checked above")
    }
}

impl ApplyEditClaim {
    /// Publishes the sole terminal result. If timeout already won, the result
    /// is rejected and the caller must not commit the mutation.
    pub(crate) fn finish(self, result: ApplyEditDecisionResult) -> bool {
        // Publish the payload before the release CAS so an acquire load that
        // observes FINISHED cannot observe an empty result.
        *self.shared.result.lock().expect("apply-edit decision lock") = Some(result);
        if self
            .shared
            .state
            .compare_exchange(CLAIMED, FINISHED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            *self.shared.result.lock().expect("apply-edit decision lock") = None;
            return false;
        }
        self.shared.completed.notify_all();
        true
    }
}

impl Drop for ApplyEditClaim {
    fn drop(&mut self) {
        if self
            .shared
            .state
            .compare_exchange(CLAIMED, FINISHED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            *self.shared.result.lock().expect("apply-edit decision lock") = Some(
                ApplyEditDecisionResult::rejected("apply claim was abandoned"),
            );
            self.shared.completed.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn claim_wins_and_timeout_waits_for_actual_result() {
        let decision = ApplyEditDecision::new();
        let claim = decision
            .claim(Instant::now() + Duration::from_secs(1))
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let timeout_decision = decision.clone();
        let timeout_barrier = Arc::clone(&barrier);
        let timeout = thread::spawn(move || {
            timeout_barrier.wait();
            timeout_decision.on_deadline()
        });
        barrier.wait();
        assert_eq!(timeout.join().unwrap(), DeadlineDecision::Claimed);
        assert!(claim.finish(ApplyEditDecisionResult::applied()));
        assert_eq!(
            decision.on_deadline(),
            DeadlineDecision::Finished(ApplyEditDecisionResult::applied())
        );
        assert_eq!(
            decision.wait_for_result(),
            ApplyEditDecisionResult::applied()
        );
    }

    #[test]
    fn timeout_wins_pending_and_claim_is_forbidden() {
        let decision = ApplyEditDecision::new();
        let barrier = Arc::new(Barrier::new(2));
        let timeout_decision = decision.clone();
        let timeout_barrier = Arc::clone(&barrier);
        let timeout = thread::spawn(move || {
            timeout_barrier.wait();
            timeout_decision.on_deadline()
        });
        barrier.wait();
        assert_eq!(timeout.join().unwrap(), DeadlineDecision::Expired);
        assert!(matches!(
            decision.claim(Instant::now() + Duration::from_secs(1)),
            Err(ClaimError::Expired)
        ));
    }

    #[test]
    fn queued_completion_at_deadline_is_preferred() {
        let decision = ApplyEditDecision::new();
        let claim = decision
            .claim(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert!(claim.finish(ApplyEditDecisionResult::applied()));
        assert_eq!(
            decision.on_deadline(),
            DeadlineDecision::Finished(ApplyEditDecisionResult::applied())
        );
    }

    #[test]
    fn expired_claim_cannot_publish_success() {
        let decision = ApplyEditDecision::new();
        let claim = decision
            .claim(Instant::now() + Duration::from_secs(1))
            .unwrap();
        assert_eq!(decision.on_deadline(), DeadlineDecision::Claimed);
        // The callback cannot expire a claimed operation; its result remains
        // authoritative and is the one the worker must await.
        assert!(claim.finish(ApplyEditDecisionResult::rejected("apply failed")));
        assert_eq!(
            decision.wait_for_result(),
            ApplyEditDecisionResult::rejected("apply failed")
        );
    }

    #[test]
    fn dropped_claim_publishes_bounded_failure() {
        let decision = ApplyEditDecision::new();
        let claim = decision
            .claim(Instant::now() + Duration::from_secs(1))
            .unwrap();
        drop(claim);
        assert_eq!(
            decision.wait_for_result(),
            ApplyEditDecisionResult::rejected("apply claim was abandoned")
        );
    }
}
