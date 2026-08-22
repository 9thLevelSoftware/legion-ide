//! The shared product-AI lane: who holds it, and who may finish it.
//!
//! Split out of `lib.rs` rather than grown there. One provider operation runs
//! at a time, and everything here exists to make that true under threads: a
//! reservation taken with the lane, an occupancy number that says which run a
//! reservation belongs to, and a result queue the app thread drains.
//!
//! The occupancy number is the part worth reading first. A worker that has been
//! cancelled can observe "not cancelled", be pre-empted, and publish after the
//! app thread has released the lane and another operation has taken it -- so
//! "am I cancelled?" is the wrong question, and "do I still hold the lane?" is
//! the right one.

use super::*;

/// Background job result for non-blocking product AI generation.
#[derive(Debug, Clone)]
pub(crate) struct ProductAiBackgroundResult {
    /// Delegate chat assistant message to finalize; empty when this is Assist.
    pub(crate) assistant_message_id: String,
    pub(crate) content_label: String,
    pub(crate) stream: Option<ProductAiStreamProjection>,
    /// When set, `poll_product_ai_stream` registers an Assist proposal on the app thread.
    pub(crate) assist_proposal: Option<AssistedEditProposalSource>,
    /// Whether a selected live provider was invoked and did not answer.
    ///
    /// The proposal says so in its own summary, and the route record has to
    /// agree: it was built with `Completed` before the worker ran, and
    /// persisting that while the proposal reports a failure leaves the audit
    /// and the artifact contradicting each other about the same run. Which one
    /// a reader believes then depends on which one they happen to open.
    pub(crate) live_failed: bool,
    /// When set, a live inline prediction finished on a worker thread.
    ///
    /// Ghost text is the smallest of these operations and was the only one that
    /// ran its provider call inline on the UI thread. The transport allows a
    /// request 120 seconds, and for all of it eframe could neither repaint nor
    /// read input -- so the `request_in_flight` state the projection already
    /// modelled, and the Cancel control that depends on it, could never be seen.
    pub(crate) inline_prediction: Option<InlinePredictionResult>,
}

/// Context retained while a live Assist proposal streams on a worker thread.
///
/// Authorization and agent Planning→Proposing run on the UI thread; completion
/// text arrives later so the renderer can poll progressive deltas and remain
/// responsive. Proposal registration happens on poll after the worker finishes.
#[derive(Debug, Clone)]
pub(crate) struct PendingAssistProposalJob {
    pub(crate) run_id: legion_protocol::AgentRunId,
    pub(crate) route_id: String,
    pub(crate) operation_class: legion_protocol::AssistedAiOperationClass,
    pub(crate) provider_class: legion_protocol::AssistedAiProviderClass,
    pub(crate) provider_route_request: legion_protocol::AssistedAiProviderRouteRequest,
    pub(crate) route_response: legion_protocol::AssistedAiProviderRouteResponse,
    pub(crate) context_manifest_projection: legion_protocol::ContextManifestProjection,
    pub(crate) privacy_inspector_projection: legion_protocol::PrivacyInspectorProjection,
    pub(crate) permission_budget_projection: legion_protocol::PermissionBudgetProjection,
    pub(crate) generated_at: TimestampMillis,
    pub(crate) event_context: EventContext,
    pub(crate) principal: PrincipalId,
    pub(crate) file_id: FileId,
    pub(crate) preconditions: ProposalVersionPreconditions,
    pub(crate) agent: AgentRuntime,
}

/// Shared live stream sink updated as SSE deltas arrive (background or progressive).
#[derive(Debug, Default)]
pub(crate) struct LiveProductAiStreamSink {
    pub(crate) projection: Mutex<ProductAiStreamProjection>,
    pub(crate) in_flight: std::sync::atomic::AtomicBool,
    /// Which occupancy of the lane is current.
    ///
    /// Incremented every time the lane is taken. A reservation records the
    /// value it was given and may only finish while it still matches, which
    /// closes a race a boolean cancellation flag cannot: the worker can read
    /// "not cancelled", be pre-empted, and by the time it publishes, the app
    /// thread has released the lane and another operation has taken it. Its
    /// result would then land against somebody else's run and clear an
    /// in-flight flag that is not its own -- making a request that is still
    /// running look finished, and letting a third into the lane behind it.
    pub(crate) generation: std::sync::atomic::AtomicU64,
    /// Completed background chat results waiting to be applied on the app thread.
    pub(crate) pending_results: Mutex<VecDeque<ProductAiBackgroundResult>>,
}

pub(crate) struct ProductAiLaneReservation {
    pub(crate) sink: Arc<LiveProductAiStreamSink>,
    pub(crate) operation: &'static str,
    /// The occupancy this reservation belongs to.
    pub(crate) generation: u64,
    pub(crate) armed: bool,
}

impl ProductAiLaneReservation {
    pub(crate) fn try_acquire(
        sink: Arc<LiveProductAiStreamSink>,
        operation: &'static str,
        provider_hint: &str,
        model_hint: &str,
    ) -> Option<Self> {
        let generation = sink.try_begin(operation, provider_hint, model_hint)?;
        Some(Self {
            sink,
            operation,
            generation,
            armed: true,
        })
    }

    pub(crate) fn sink(&self) -> Arc<LiveProductAiStreamSink> {
        self.sink.clone()
    }

    /// Give the lane up without finishing it, because somebody else already has.
    ///
    /// A cancelled prediction releases the lane from the app thread so the rest
    /// of the product is not blocked behind a provider call nobody is waiting
    /// for. The worker still holds this reservation until its request returns,
    /// and it must not release the lane a second time -- by then it may belong
    /// to another operation.
    pub(crate) fn abandon(mut self) {
        self.armed = false;
    }

    pub(crate) fn finish(mut self, completion: Option<&ProductChatCompletion>) {
        self.armed = false;
        self.sink.finish(completion, self.operation);
    }

    pub(crate) fn finish_background(
        mut self,
        result: ProductAiBackgroundResult,
        completion: Option<&ProductChatCompletion>,
    ) {
        self.armed = false;
        // Only while this reservation still holds the lane.
        //
        // A cancelled worker can read "not cancelled", be pre-empted, and reach
        // here after the app thread released the lane and another operation
        // took it. Publishing then lands a stale result against somebody
        // else's run and clears an in-flight flag that is not this worker's --
        // so a request still in progress looks finished, and a third is let in
        // behind it.
        if !self.sink.owns_lane(self.generation) {
            return;
        }
        self.sink
            .finish_background(result, completion, self.operation);
    }
}

impl Drop for ProductAiLaneReservation {
    fn drop(&mut self) {
        if self.armed {
            self.sink.finish(None, self.operation);
        }
    }
}

impl LiveProductAiStreamSink {
    /// Take the lane, returning the occupancy number if it was free.
    pub(crate) fn try_begin(
        &self,
        operation: &str,
        provider_hint: &str,
        model_hint: &str,
    ) -> Option<u64> {
        let Ok(pending) = self.pending_results.lock() else {
            return None;
        };
        if !pending.is_empty()
            || self
                .in_flight
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_err()
        {
            return None;
        }
        // A new occupancy, taken under the same exchange that took the lane.
        let generation = self
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .wrapping_add(1);
        let Ok(mut guard) = self.projection.lock() else {
            self.in_flight
                .store(false, std::sync::atomic::Ordering::SeqCst);
            return None;
        };
        *guard = ProductAiStreamProjection {
            provider_id: provider_hint.to_string(),
            model: model_hint.to_string(),
            operation: operation.to_string(),
            chunks: Vec::new(),
            streamed: false,
            in_flight: true,
            text_preview: String::new(),
        };
        Some(generation)
    }

    /// Whether `generation` is still the occupancy holding the lane.
    /// Free the lane from the app thread and end the current occupancy.
    pub(crate) fn release_lane(&self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.finish(None, "");
    }

    pub(crate) fn owns_lane(&self, generation: u64) -> bool {
        self.generation.load(std::sync::atomic::Ordering::SeqCst) == generation
    }

    pub(crate) fn push_delta(&self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.projection.lock() {
            guard.chunks.push(delta.to_string());
            guard.streamed = guard.chunks.len() > 1 || guard.streamed;
            guard.in_flight = true;
            let joined = guard.chunks.join("");
            guard.text_preview = joined.chars().take(480).collect();
        }
    }

    pub(crate) fn finish(&self, completion: Option<&ProductChatCompletion>, operation: &str) {
        if let Ok(mut guard) = self.projection.lock() {
            if let Some(completion) = completion {
                *guard = product_stream_from_completion(completion, operation);
            } else {
                guard.in_flight = false;
            }
        }
        self.in_flight
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn snapshot(&self) -> ProductAiStreamProjection {
        self.projection
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub(crate) fn is_in_flight(&self) -> bool {
        self.in_flight.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn has_pending_results(&self) -> bool {
        self.pending_results
            .lock()
            .map(|queue| !queue.is_empty())
            .unwrap_or(true)
    }

    pub(crate) fn mode_allows_active_operation(&self, mode: AppProductMode) -> bool {
        if !self.is_in_flight() && !self.has_pending_results() {
            return true;
        }
        let Ok(guard) = self.projection.lock() else {
            return false;
        };
        match guard.operation.as_str() {
            "delegate.chat" => mode.allows_delegate(),
            "assist.proposal" | "assist.inline_prediction" => mode.allows_assist(),
            _ => false,
        }
    }

    /// Publish the final stream projection and enqueue its app-thread result
    /// before making the operation observable as no longer in flight. This
    /// ordering closes the Manual-transition race between provider completion
    /// and result handoff.
    pub(crate) fn finish_background(
        &self,
        result: ProductAiBackgroundResult,
        completion: Option<&ProductChatCompletion>,
        operation: &str,
    ) {
        let Ok(mut queue) = self.pending_results.lock() else {
            return;
        };
        if let Ok(mut guard) = self.projection.lock() {
            if let Some(completion) = completion {
                *guard = product_stream_from_completion(completion, operation);
            } else {
                guard.in_flight = false;
            }
        }
        queue.push_back(result);
        self.in_flight
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn take_background_results(&self) -> Vec<ProductAiBackgroundResult> {
        self.pending_results
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default()
    }
}
