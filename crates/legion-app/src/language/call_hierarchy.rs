//! Call hierarchy: who calls this symbol, and what does this symbol call.
//!
//! The protocol layer for this has been complete and contract-tested in
//! `legion-lsp` for some time while all three intents routed to
//! `AppCommandRequest::Noop`. The `intent-reachability` gate is what made that
//! visible rather than invisible; backlog task P2.F1.T6 tracks it.
//!
//! ## Two round trips, one question
//!
//! LSP does not answer "who calls the thing under my caret" in one request. It
//! answers:
//!
//! 1. `textDocument/prepareCallHierarchy` — resolve a position to a symbol item;
//! 2. `callHierarchy/incomingCalls` or `outgoingCalls` — ask about that item.
//!
//! The direction is chosen at step one and needed at step two, so it waits in
//! `pending_call_hierarchy` in between. Making the caller issue both instead
//! would put an LSP sequencing detail into the intent vocabulary, where a user
//! gesture has no business knowing about it.
//!
//! ## Why the rows are locations
//!
//! A call is a place in a file, exactly as a reference is, so the rows are
//! `LanguageLocationProjection` and land in the panel that already lists
//! locations. A dedicated call-hierarchy panel would have meant building a new
//! surface for a row type the product already renders — and an unbuilt surface
//! is how a feature ends up complete and unreachable, which is the defect this
//! task exists to retire.
//!
//! What a location row cannot say is which question produced it, so the
//! direction rides alongside in the projection: "who calls this" and "what does
//! this call" are opposite answers that look identical in a list.

use legion_protocol::{
    BufferId, CallHierarchyDirection, LanguageLocationProjection, LspCallHierarchyIncomingCall,
    LspCallHierarchyItem, LspCallHierarchyOutgoingCall, ProtocolTextRange, TextCoordinate,
};

/// A call-hierarchy request waiting on its `prepareCallHierarchy` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCallHierarchy {
    /// Buffer the request was issued for.
    pub buffer_id: BufferId,
    /// Direction to ask for once an item comes back.
    ///
    /// `None` means the caller wanted only the prepare step: the symbol under
    /// the caret, with no follow-up.
    pub direction: Option<CallHierarchyDirection>,
}

/// Maximum rows kept from one call-hierarchy response.
///
/// The same order of magnitude as the reference cap. A symbol with more callers
/// than this is one where a flat list is the wrong tool anyway, and an
/// unbounded projection makes the renderer everyone else's problem.
pub const CALL_HIERARCHY_ROW_CAP: usize = 250;

/// Build the row label for a call.
///
/// The symbol name leads because it is what a reader scans for. Detail —
/// rust-analyzer puts the containing module or the signature here — follows
/// when the server supplies it, since two functions of the same name in
/// different modules are otherwise indistinguishable in the list.
pub fn call_row_label(item: &LspCallHierarchyItem) -> String {
    match item.detail.as_deref().map(str::trim) {
        Some(detail) if !detail.is_empty() => format!("{} — {detail}", item.name),
        _ => item.name.clone(),
    }
}

/// The item a prepare response resolved to, if any.
///
/// An empty list is the server saying "there is no symbol here", which is the
/// ordinary outcome of asking on whitespace. It must not be reported as a
/// failure, and must not leave a caller waiting for a second request that will
/// never be issued.
pub fn first_item(items: &[LspCallHierarchyItem]) -> Option<&LspCallHierarchyItem> {
    items.first()
}

/// The `textDocument/prepareCallHierarchy` parameters for a caret position.
pub fn prepare_params(uri: &str, position: TextCoordinate) -> serde_json::Value {
    serde_json::json!({
        "textDocument": { "uri": uri },
        "position": { "line": position.line, "character": position.character },
    })
}

/// The `callHierarchy/incomingCalls` and `outgoingCalls` parameters.
///
/// `data` and `detail` are forwarded when present. `data` is opaque and
/// server-owned — rust-analyzer round-trips its own resolution state through it
/// — so dropping it makes the follow-up request ambiguous for any server that
/// relies on it.
pub fn call_params(item: &LspCallHierarchyItem) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    fields.insert("name".to_string(), item.name.clone().into());
    fields.insert("kind".to_string(), item.kind.into());
    fields.insert("uri".to_string(), item.uri.clone().into());
    fields.insert("range".to_string(), range_json(&item.range));
    fields.insert(
        "selectionRange".to_string(),
        range_json(&item.selection_range),
    );
    if let Some(data) = &item.data {
        fields.insert("data".to_string(), data.clone());
    }
    if let Some(detail) = &item.detail {
        fields.insert("detail".to_string(), detail.clone().into());
    }
    serde_json::json!({ "item": serde_json::Value::Object(fields) })
}

fn range_json(range: &ProtocolTextRange) -> serde_json::Value {
    serde_json::json!({
        "start": { "line": range.start.line, "character": range.start.character },
        "end": { "line": range.end.line, "character": range.end.character },
    })
}

/// A stable identifier for a call row.
///
/// Built from the item's URI and the call site rather than from a counter, so
/// the same call keeps the same id across a refresh and a selection survives
/// one.
pub fn call_row_id(uri: &str, range: &ProtocolTextRange) -> String {
    format!("call:{uri}:{}:{}", range.start.line, range.start.character)
}

/// Summary line for the projection status message.
pub fn status_message(direction: CallHierarchyDirection, count: usize) -> String {
    match direction {
        CallHierarchyDirection::Incoming => format!("LSP incoming calls merged ({count} callers)"),
        CallHierarchyDirection::Outgoing => format!("LSP outgoing calls merged ({count} callees)"),
    }
}

/// Rows for an `incomingCalls` response.
pub fn rows_from_incoming(
    calls: &[LspCallHierarchyIncomingCall],
) -> Vec<LanguageLocationProjection> {
    rows(calls.iter().map(|call| (&call.from, &call.from_ranges)))
}

/// Rows for an `outgoingCalls` response.
pub fn rows_from_outgoing(
    calls: &[LspCallHierarchyOutgoingCall],
) -> Vec<LanguageLocationProjection> {
    rows(calls.iter().map(|call| (&call.to, &call.from_ranges)))
}

/// One row per call site, capped.
///
/// Per site rather than per symbol: a caller that invokes the symbol three
/// times is three places a reader may want to go, and collapsing them would
/// make the list shorter and less useful.
fn rows<'a>(
    calls: impl Iterator<Item = (&'a LspCallHierarchyItem, &'a Vec<ProtocolTextRange>)>,
) -> Vec<LanguageLocationProjection> {
    let mut rows = Vec::new();
    for (item, from_ranges) in calls {
        if from_ranges.is_empty() {
            if rows.len() >= CALL_HIERARCHY_ROW_CAP {
                break;
            }
            // A call with no ranges still names a symbol worth listing; the
            // server simply did not say where in the caller it happens.
            // Dropping it would silently lose a caller. Marked degraded,
            // because pointing at the symbol's own declaration is not the same
            // answer as pointing at the call site.
            rows.push(row(item, &item.selection_range, true));
            continue;
        }
        for range in from_ranges {
            if rows.len() >= CALL_HIERARCHY_ROW_CAP {
                return rows;
            }
            rows.push(row(item, range, false));
        }
    }
    rows
}

fn row(
    item: &LspCallHierarchyItem,
    range: &ProtocolTextRange,
    degraded: bool,
) -> LanguageLocationProjection {
    LanguageLocationProjection {
        location_id: call_row_id(&item.uri, range),
        // Left unresolved on purpose: the row carries the URI's position, and
        // resolving a URI to a `FileId` and a disclosable path is the caller's
        // job under its own workspace and redaction rules, not this module's.
        file_id: None,
        path: None,
        range: Some(*range),
        label: call_row_label(item),
        degraded,
        schema_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinate(line: u32, character: u32) -> TextCoordinate {
        TextCoordinate {
            line,
            character,
            byte_offset: None,
            utf16_offset: None,
        }
    }

    fn range(line: u32, character: u32) -> ProtocolTextRange {
        ProtocolTextRange {
            start: coordinate(line, character),
            end: coordinate(line, character + 4),
        }
    }

    fn item(name: &str, detail: Option<&str>) -> LspCallHierarchyItem {
        LspCallHierarchyItem {
            name: name.to_string(),
            kind: 12,
            uri: format!("file:///workspace/{name}.rs"),
            range: range(1, 0),
            selection_range: range(1, 4),
            detail: detail.map(str::to_string),
            data: None,
        }
    }

    #[test]
    fn every_call_site_gets_its_own_row() {
        // A caller that invokes the symbol three times is three places a reader
        // may want to go. Collapsing to one row per symbol would make the list
        // shorter and lose two of them.
        let calls = vec![LspCallHierarchyIncomingCall {
            from: item("caller", None),
            from_ranges: vec![range(10, 4), range(20, 8), range(30, 2)],
        }];
        let rows = rows_from_incoming(&calls);
        assert_eq!(rows.len(), 3, "one row per call site");
        let ids: std::collections::BTreeSet<_> = rows.iter().map(|r| &r.location_id).collect();
        assert_eq!(ids.len(), 3, "each site needs a distinct id: {rows:?}");
    }

    #[test]
    fn a_call_with_no_ranges_is_kept_and_marked_degraded() {
        // Dropping it would silently lose a caller. Keeping it while claiming
        // the declaration is the call site would be a quieter lie.
        let calls = vec![LspCallHierarchyIncomingCall {
            from: item("silent", None),
            from_ranges: Vec::new(),
        }];
        let rows = rows_from_incoming(&calls);
        assert_eq!(rows.len(), 1, "the caller is still listed");
        assert!(
            rows[0].degraded,
            "a row pointing at the declaration rather than the call site is degraded"
        );
    }

    #[test]
    fn rows_are_capped() {
        let calls = vec![LspCallHierarchyIncomingCall {
            from: item("hot", None),
            from_ranges: (0..(CALL_HIERARCHY_ROW_CAP as u32 * 2))
                .map(|line| range(line, 0))
                .collect(),
        }];
        assert_eq!(rows_from_incoming(&calls).len(), CALL_HIERARCHY_ROW_CAP);
    }

    #[test]
    fn detail_disambiguates_same_named_symbols() {
        // Two `new` functions in different modules are the same string without
        // it, and a list of identical rows helps nobody.
        assert_eq!(
            call_row_label(&item("new", Some("billing::Invoice"))),
            "new — billing::Invoice"
        );
        assert_eq!(call_row_label(&item("new", None)), "new");
        assert_eq!(
            call_row_label(&item("new", Some("   "))),
            "new",
            "whitespace detail must not produce a dangling separator"
        );
    }

    #[test]
    fn outgoing_calls_read_the_callee_side() {
        // `incomingCalls` reports `from` and `outgoingCalls` reports `to`.
        // Reading the wrong one compiles and produces a plausible, wrong list.
        let calls = vec![LspCallHierarchyOutgoingCall {
            to: item("callee", None),
            from_ranges: vec![range(5, 0)],
        }];
        let rows = rows_from_outgoing(&calls);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "callee");
    }

    #[test]
    fn opaque_server_data_survives_into_the_follow_up_request() {
        // rust-analyzer round-trips resolution state through `data`. Dropping
        // it makes the second request ambiguous for servers that rely on it,
        // and the failure would look like "the server returned nothing".
        let mut with_data = item("resolved", Some("module::path"));
        with_data.data = Some(serde_json::json!({ "opaque": 7 }));
        let params = call_params(&with_data);
        assert_eq!(params["item"]["data"]["opaque"], 7);
        assert_eq!(params["item"]["detail"], "module::path");
        assert_eq!(params["item"]["selectionRange"]["start"]["line"], 1);
    }

    #[test]
    fn an_empty_prepare_response_resolves_to_nothing() {
        // Asking on whitespace is ordinary. It must not read as a failure, and
        // must not leave a caller waiting for a follow-up never issued.
        assert!(first_item(&[]).is_none());
    }
}
