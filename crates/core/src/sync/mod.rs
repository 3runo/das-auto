//! Sync domain logic (T1-2).
//!
//! The Supabase transport lives in `net` (T1-3); this module holds the
//! platform-independent rules: conflict resolution and offline op-queue
//! normalization. The persistent queue *storage* is T2-3 — the op shape and
//! merge semantics are pinned here so the later task plugs into them.

use std::cmp::Ordering;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::ValidationError;
use crate::store::StoreError;
use crate::{DateTime, Utc};

/// A pending local write waiting for the cloud (offline-first, plan.md §5).
///
/// Only local stores know the data at the moment of the change; the cloud
/// cannot see offline edits, so replaying the op is always valid locally —
/// `apply` on the local store is a no-op (idempotent upsert) that keeps the
/// cache consistent with the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SyncOp {
    /// Create or replace a whole row (last-write-wins on replay).
    Upsert {
        entity: SyncEntity,
        row: serde_json::Value,
    },
    /// Delete a row by id (idempotent: deleting a missing row is fine).
    Delete { entity: SyncEntity, id: Uuid },
}

/// Row-level syncable entities. (Vehicles, expenses, documents, deadlines,
/// saved_parts_searches — the same five `LocalStore` persists.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncEntity {
    Vehicle,
    Expense,
    Document,
    Deadline,
    SavedPartsSearch,
}

impl SyncOp {
    pub fn upsert(entity: SyncEntity, row: serde_json::Value) -> Self {
        SyncOp::Upsert { entity, row }
    }

    pub fn delete(entity: SyncEntity, id: Uuid) -> Self {
        SyncOp::Delete { entity, id }
    }

    pub fn entity(&self) -> SyncEntity {
        match self {
            SyncOp::Upsert { entity, .. } | SyncOp::Delete { entity, .. } => *entity,
        }
    }

    pub fn id(&self) -> Option<Uuid> {
        match self {
            SyncOp::Delete { id, .. } => Some(*id),
            SyncOp::Upsert { row, .. } => row
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::from_str(s).ok()),
        }
    }
}

/// Outcome of merging a local and a remote row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeOutcome {
    /// Local and remote are the same row — nothing to do.
    Identical,
    /// Local is newer — upload wins.
    LocalWins,
    /// Remote is newer — overwrite the cache with remote.
    RemoteWins,
    /// Equal `updated_at` but different content — deterministic tie-break
    /// (local wins to preserve user intent; documented in decisions.md D15).
    LocalWinsTie,
}

/// Last-write-wins by `updated_at` (plan.md §5: no row-level locking).
///
/// `local` is the cached row, `remote` the row just fetched from Supabase.
/// When the timestamps are equal the rows are compared in full (including
/// `updated_at`, which then carries the same value on both sides): equal →
/// `Identical`, different → `LocalWinsTie` (local wins the tie to preserve
/// user intent; decisions.md D15).
pub fn resolve_conflict(
    local_updated_at: DateTime<Utc>,
    remote_updated_at: DateTime<Utc>,
    local: &serde_json::Value,
    remote: &serde_json::Value,
) -> MergeOutcome {
    match local_updated_at.cmp(&remote_updated_at) {
        Ordering::Greater => MergeOutcome::LocalWins,
        Ordering::Less => MergeOutcome::RemoteWins,
        Ordering::Equal => {
            if local == remote {
                MergeOutcome::Identical
            } else {
                MergeOutcome::LocalWinsTie
            }
        }
    }
}

/// Offline queue invariants (rules.md §4: the queue must never corrupt).
pub fn validate_queue(ops: &[SyncOp]) -> Result<(), ValidationError> {
    for (index, op) in ops.iter().enumerate() {
        match op {
            SyncOp::Upsert { entity: _, row } => {
                let id = row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::from_str(s).ok());
                if id.is_none() {
                    return Err(ValidationError::new(
                        "sync_queue",
                        format!("op #{index} upsert is missing a valid `id`"),
                    ));
                }
            }
            SyncOp::Delete { .. } => {
                // Delete always carries a valid Uuid by construction.
            }
        }
    }
    Ok(())
}

/// The op that should be persisted next. The queue is FIFO: the oldest
/// pending op is the head. (Actual replay ordering vs. cloud is T2-3; this
/// pins the invariant that replay walks the queue in insertion order.)
pub fn next_op(ops: &[SyncOp]) -> Option<&SyncOp> {
    ops.first()
}

/// Idempotency contract for `Upsert` replay: applying the same op twice
/// must not duplicate rows. This is a documentation anchor for the T2-3
/// replay engine; `LocalStore` upserts already satisfy it (see store tests).
///
/// Returns the merged queue: `pending` with any op whose `id`+`entity`
/// appears in `completed` removed, in original order.
pub fn filter_completed(pending: &[SyncOp], completed: &[(SyncEntity, Uuid)]) -> Vec<SyncOp> {
    pending
        .iter()
        .filter(|op| {
            let id = match op {
                SyncOp::Delete { id, .. } => Some(*id),
                SyncOp::Upsert { row, .. } => row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::from_str(s).ok()),
            };
            match (op.entity(), id) {
                (entity, Some(id)) => !completed.contains(&(entity, id)),
                (_, None) => true, // malformed upsert: keep, validate_queue flags it
            }
        })
        .cloned()
        .collect()
}

/// Stores corrupt queue data as `StoreError::InvalidData`; the caller
/// (T2-3) decides whether to quarantine. Exposed here so the sync layer and
/// the store layer share one taxonomy of failure.
pub fn is_data_error(err: &StoreError) -> bool {
    matches!(err, StoreError::InvalidData(_))
}

/// The offline write queue (T1-2 foundation; the persistent queue with
/// retry/backoff/replay is T2-3).
///
/// Invariants (rules.md §4): malformed ops are rejected at the boundary —
/// the queue can never hold an upsert without a valid `id` — and replay
/// walks the queue in insertion order (FIFO).
pub trait SyncQueue {
    /// Enqueue a pending write. Rejects malformed ops (see [`validate_queue`]).
    fn enqueue(&mut self, op: SyncOp) -> Result<(), ValidationError>;

    /// The next op to replay (oldest first), if any.
    fn next(&self) -> Option<&SyncOp>;

    /// Remove and return the head op once the cloud has confirmed it.
    fn pop(&mut self) -> Option<SyncOp>;

    /// Pending op count — drives the "N pendentes" indicator (plan.md §5).
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory queue: the test fake and the app's runtime queue shape.
/// `InMemorySyncQueue` is what T1-3 wires in; T2-3 swaps in the persistent
/// implementation behind this same trait.
#[derive(Debug, Default)]
pub struct InMemorySyncQueue {
    ops: Vec<SyncOp>,
}

impl InMemorySyncQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// The full pending queue (oldest first).
    pub fn ops(&self) -> &[SyncOp] {
        &self.ops
    }
}

impl SyncQueue for InMemorySyncQueue {
    fn enqueue(&mut self, op: SyncOp) -> Result<(), ValidationError> {
        validate_queue(std::slice::from_ref(&op))?;
        self.ops.push(op);
        Ok(())
    }

    fn next(&self) -> Option<&SyncOp> {
        next_op(&self.ops)
    }

    fn pop(&mut self) -> Option<SyncOp> {
        if self.ops.is_empty() {
            return None;
        }
        Some(self.ops.remove(0))
    }

    fn len(&self) -> usize {
        self.ops.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn utc(ts: &str) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ").unwrap(),
        )
    }

    fn row(plate: &str, updated_at: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "plate": plate,
            "updated_at": updated_at,
        })
    }

    #[test]
    fn newer_local_wins() {
        let local = row("AB1C2D3", "2026-11-05T12:00:01Z");
        let remote = row("AB1C2D3", "2026-11-05T12:00:00Z");
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:01Z"),
                utc("2026-11-05T12:00:00Z"),
                &local,
                &remote
            ),
            MergeOutcome::LocalWins
        );
    }

    #[test]
    fn newer_remote_wins() {
        let local = row("AB1C2D3", "2026-11-05T12:00:00Z");
        let remote = row("AB1C2D4", "2026-11-05T12:00:05Z");
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:00Z"),
                utc("2026-11-05T12:00:05Z"),
                &local,
                &remote
            ),
            MergeOutcome::RemoteWins
        );
    }

    #[test]
    fn identical_rows_are_identical() {
        let local = row("AB1C2D3", "2026-11-05T12:00:00Z");
        let remote = row("AB1C2D3", "2026-11-05T12:00:00Z");
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:00Z"),
                utc("2026-11-05T12:00:00Z"),
                &local,
                &remote
            ),
            MergeOutcome::Identical
        );
    }

    #[test]
    fn same_timestamp_same_content_is_identical() {
        // rows equal (JSON objects compare by content, key order aside)
        let local = row("AB1C2D3", "2026-11-05T12:00:00Z");
        let remote = serde_json::json!({
            "plate": "AB1C2D3",
            "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "updated_at": "2026-11-05T12:00:00Z"
        });
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:00Z"),
                utc("2026-11-05T12:00:00Z"),
                &local,
                &remote
            ),
            MergeOutcome::Identical
        );
    }

    #[test]
    fn same_timestamp_different_content_ties_to_local() {
        let local = row("AB1C2D3", "2026-11-05T12:00:00Z");
        let remote = row("AB1C2D4", "2026-11-05T12:00:00Z");
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:00Z"),
                utc("2026-11-05T12:00:00Z"),
                &local,
                &remote
            ),
            MergeOutcome::LocalWinsTie
        );
    }

    #[test]
    fn equality_includes_updated_at() {
        // full-row comparison: an embedded ts difference (with the recency
        // arguments equal, i.e. corrupt/foreign data) is a real difference
        let local = row("AB1C2D3", "2026-11-05T12:00:00Z");
        let remote = row("AB1C2D3", "2026-11-05T12:00:09Z");
        assert_eq!(
            resolve_conflict(
                utc("2026-11-05T12:00:00Z"),
                utc("2026-11-05T12:00:00Z"),
                &local,
                &remote
            ),
            MergeOutcome::LocalWinsTie
        );
    }

    fn upsert_op(plate: &str) -> SyncOp {
        SyncOp::upsert(SyncEntity::Vehicle, row(plate, "2026-11-05T12:00:00Z"))
    }

    #[test]
    fn op_serializes_with_tagged_shape() {
        let op = upsert_op("AB1C2D3");
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "upsert");
        assert_eq!(json["entity"], "vehicle");
        assert_eq!(json["row"]["plate"], "AB1C2D3");

        let del = SyncOp::delete(SyncEntity::Expense, Uuid::from_u128(7));
        let json = serde_json::to_value(&del).unwrap();
        assert_eq!(json["op"], "delete");
        assert_eq!(json["entity"], "expense");
    }

    #[test]
    fn op_round_trips() {
        let ops = vec![
            upsert_op("AB1C2D3"),
            SyncOp::delete(SyncEntity::Document, Uuid::from_u128(9)),
        ];
        let json = serde_json::to_string(&ops).unwrap();
        let back: Vec<SyncOp> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ops);
    }

    #[test]
    fn op_exposes_entity_and_id() {
        let up = upsert_op("AB1C2D3");
        assert_eq!(up.entity(), SyncEntity::Vehicle);
        assert_eq!(
            up.id(),
            Some(Uuid::from_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap())
        );
        let del = SyncOp::delete(SyncEntity::Deadline, Uuid::from_u128(3));
        assert_eq!(del.entity(), SyncEntity::Deadline);
        assert_eq!(del.id(), Some(Uuid::from_u128(3)));
    }

    #[test]
    fn queue_validates_well_formed_ops() {
        let ops = vec![
            upsert_op("AB1C2D3"),
            SyncOp::delete(SyncEntity::Vehicle, Uuid::from_u128(1)),
        ];
        validate_queue(&ops).unwrap();
    }

    #[test]
    fn queue_rejects_upsert_without_id() {
        let bad = SyncOp::Upsert {
            entity: SyncEntity::Vehicle,
            row: serde_json::json!({ "plate": "AB1C2D3" }),
        };
        let err = validate_queue(&[bad]).unwrap_err();
        assert_eq!(err.field(), "sync_queue");
    }

    #[test]
    fn queue_rejects_upsert_with_malformed_id() {
        let bad = SyncOp::Upsert {
            entity: SyncEntity::Vehicle,
            row: serde_json::json!({ "id": "not-a-uuid", "plate": "AB1C2D3" }),
        };
        assert!(validate_queue(&[bad]).is_err());
    }

    #[test]
    fn next_op_is_the_head_of_the_queue() {
        let a = upsert_op("AB1C2D3");
        let b = upsert_op("XY9Z8W7");
        assert_eq!(next_op(&[a.clone(), b.clone()]), Some(&a));
        assert_eq!(next_op(std::slice::from_ref(&a)), Some(&a));
        assert_eq!(next_op(&[]), None);
    }

    #[test]
    fn filter_completed_removes_done_ops_in_place() {
        let a = upsert_op("AB1C2D3");
        let del = SyncOp::delete(SyncEntity::Expense, Uuid::from_u128(7));
        let pending = vec![a.clone(), del.clone(), upsert_op("XY9Z8W7")];
        let completed = vec![(SyncEntity::Expense, Uuid::from_u128(7))];
        let remaining = filter_completed(&pending, &completed);
        assert_eq!(remaining, vec![a, upsert_op("XY9Z8W7")]);
    }

    #[test]
    fn filter_completed_keeps_ops_completed_on_other_entities() {
        // a completed Expense op does not filter a Vehicle op with the same id
        let a = upsert_op("AB1C2D3");
        let completed = vec![(SyncEntity::Expense, Uuid::from_u128(7))];
        let del = SyncOp::delete(SyncEntity::Vehicle, Uuid::from_u128(7));
        assert_eq!(
            filter_completed(&[a.clone(), del.clone()], &completed),
            vec![a, del]
        );
    }

    #[test]
    fn is_data_error_classifies_store_failures() {
        use crate::store::{LocalStore, MemoryStore};
        let store = MemoryStore::new();
        store.set_raw("dasauto:vehicles", "junk").unwrap();
        assert!(is_data_error(&store.list_vehicles().unwrap_err()));
        // backend errors are a different class (unreachable in MemoryStore,
        // but the classifier must not mislabel corrupt data)
        assert!(!is_data_error(&StoreError::Backend("disk full".into())));
    }

    #[test]
    fn queue_rejects_malformed_ops_and_keeps_the_good_ones() {
        let mut q = InMemorySyncQueue::new();
        assert!(q.is_empty());
        // upsert without a valid id is rejected at the boundary (rules.md §4)
        let bad = SyncOp::upsert(
            SyncEntity::Vehicle,
            serde_json::json!({ "plate": "AB1C2D3" }),
        );
        assert!(q.enqueue(bad).is_err());
        assert!(q.is_empty());
        // a valid op still goes through afterwards
        let good = upsert_op("AB1C2D3");
        q.enqueue(good.clone()).unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q.next(), Some(&good));
    }

    #[test]
    fn queue_pops_in_fifo_order() {
        let mut q = InMemorySyncQueue::new();
        let a = upsert_op("AB1C2D3");
        let b = upsert_op("XY9Z8W7");
        q.enqueue(a.clone()).unwrap();
        q.enqueue(b.clone()).unwrap();
        assert_eq!(q.next(), Some(&a));
        assert_eq!(q.pop(), Some(a));
        assert_eq!(q.next(), Some(&b));
        assert_eq!(q.pop(), Some(b));
        assert!(q.is_empty());
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn queue_len_tracks_pending_ops() {
        let mut q = InMemorySyncQueue::new();
        q.enqueue(upsert_op("AB1C2D3")).unwrap();
        q.enqueue(SyncOp::delete(SyncEntity::Expense, Uuid::from_u128(9)))
            .unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q.ops().len(), 2);
        q.pop();
        assert_eq!(q.len(), 1);
    }
}
