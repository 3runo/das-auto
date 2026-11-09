//! Integration tests over the JSON fixtures in `fixtures/` (T1-2).
//!
//! These pin the wire format of the app's local cache and — by extension —
//! of the Supabase rows it will sync, so the T1-3/T2-3 work can rely on a
//! known-good schema (rules.md §4: test against fixtures).

use dasauto_core::entities::{Deadline, Document, Expense, SavedPartsSearch, Vehicle};
use dasauto_core::store::{keys, LocalStore, MemoryStore};
use std::str::FromStr;
use uuid::Uuid;

const USER_ID: &str = "00000000-0000-0000-0000-000000000001";
const GOL_ID: &str = "00000000-0000-0000-0000-000000000002";

fn gol_id() -> Uuid {
    Uuid::from_str(GOL_ID).unwrap()
}

#[test]
fn valid_vehicles_deserialize_and_pass_validation() {
    let gol: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-gol.json")).unwrap();
    assert_eq!(gol.make, "Volkswagen");
    assert_eq!(gol.plate, "ABC1D23");
    assert_eq!(gol.id, Uuid::from_str(GOL_ID).unwrap());
    assert_eq!(gol.user_id, Uuid::from_str(USER_ID).unwrap());
    gol.validate().unwrap();

    let uno: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-uno.json")).unwrap();
    uno.validate().unwrap();
    // `version`/`renavam` are optional; `fuel_type`/`state_uo` default
    assert!(uno.version.is_none());
    assert!(uno.renavam.is_none());
    assert_eq!(uno.fuel_type, dasauto_core::entities::FuelType::Gasoline);
    assert_eq!(uno.state_uo, "SP");
}

#[test]
fn invalid_vehicles_are_rejected_with_field_errors() {
    let bad_year: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-invalid-year.json")).unwrap();
    let err = bad_year.validate().unwrap_err();
    assert_eq!(err.field(), "year");

    let bad_plate: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-invalid-plate.json")).unwrap();
    // dashes are stripped by normalization? No — normalization only strips
    // whitespace; the dash survives and fails the alphanumeric check (and
    // the length check would fail too: "TOYOCORLA1" is 10 chars).
    let err = bad_plate.validate().unwrap_err();
    assert_eq!(err.field(), "plate");
}

#[test]
fn vehicle_children_deserialize_with_matching_vehicle_id() {
    let expenses: Vec<Expense> =
        serde_json::from_str(include_str!("../../../fixtures/expenses-gol.json")).unwrap();
    assert_eq!(expenses.len(), 2);
    for e in &expenses {
        assert_eq!(e.vehicle_id, gol_id());
        e.validate().unwrap();
    }
    // money stays integer centavos end-to-end
    assert_eq!(expenses[0].amount_cents, 25_000);
    assert_eq!(expenses[1].amount_cents, 28_700);

    let documents: Vec<Document> =
        serde_json::from_str(include_str!("../../../fixtures/documents-gol.json")).unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].vehicle_id, gol_id());
    documents[0].validate().unwrap();

    let deadlines: Vec<Deadline> =
        serde_json::from_str(include_str!("../../../fixtures/deadlines-gol.json")).unwrap();
    assert_eq!(deadlines.len(), 2);
    for d in &deadlines {
        assert_eq!(d.vehicle_id, gol_id());
        d.validate().unwrap();
    }

    let searches: Vec<SavedPartsSearch> = serde_json::from_str(include_str!(
        "../../../fixtures/saved_parts_searches-gol.json"
    ))
    .unwrap();
    assert_eq!(searches.len(), 1);
    assert_eq!(searches[0].vehicle_id, gol_id());
    searches[0].validate().unwrap();
    // the snapshot is opaque JSON (jsonb in Postgres) — assert one field
    assert_eq!(searches[0].snapshot[0]["id"], "MLB0001");
}

#[test]
fn fixtures_round_trip_through_memory_store() {
    let store = MemoryStore::new();

    let gol: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-gol.json")).unwrap();
    store.upsert_vehicle(&gol).unwrap();

    let expenses: Vec<Expense> =
        serde_json::from_str(include_str!("../../../fixtures/expenses-gol.json")).unwrap();
    for e in &expenses {
        store.upsert_expense(e).unwrap();
    }
    let documents: Vec<Document> =
        serde_json::from_str(include_str!("../../../fixtures/documents-gol.json")).unwrap();
    for d in &documents {
        store.upsert_document(d).unwrap();
    }
    let deadlines: Vec<Deadline> =
        serde_json::from_str(include_str!("../../../fixtures/deadlines-gol.json")).unwrap();
    for d in &deadlines {
        store.upsert_deadline(d).unwrap();
    }
    let searches: Vec<SavedPartsSearch> = serde_json::from_str(include_str!(
        "../../../fixtures/saved_parts_searches-gol.json"
    ))
    .unwrap();
    for s in &searches {
        store.upsert_saved_parts_search(s).unwrap();
    }

    // read everything back — equality means the JSON round-trip is lossless
    assert_eq!(store.get_vehicle(&gol_id()).unwrap().as_ref(), Some(&gol));
    assert_eq!(store.list_expenses(&gol_id()).unwrap(), expenses);
    assert_eq!(store.list_documents(&gol_id()).unwrap(), documents);
    assert_eq!(store.list_deadlines(&gol_id()).unwrap(), deadlines);
    assert_eq!(
        store.list_saved_parts_searches(&gol_id()).unwrap(),
        searches
    );

    // the raw cache layout matches the keys namespace from the plan
    let raw = store.get_raw(keys::VEHICLES).unwrap().unwrap();
    assert!(raw.contains(&gol.id.to_string()));
    assert!(store.get_raw(&keys::expenses(&gol_id())).unwrap().is_some());
    assert!(store
        .get_raw(&keys::documents(&gol_id()))
        .unwrap()
        .is_some());
    assert!(store
        .get_raw(&keys::deadlines(&gol_id()))
        .unwrap()
        .is_some());
    assert!(store
        .get_raw(&keys::saved_parts_searches(&gol_id()))
        .unwrap()
        .is_some());

    // a second vehicle's cache keys are disjoint
    let uno: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-uno.json")).unwrap();
    store.upsert_vehicle(&uno).unwrap();
    assert!(store.list_expenses(&uno.id).unwrap().is_empty());
    assert_eq!(store.list_vehicles().unwrap().len(), 2);
}

#[test]
fn sync_ops_built_from_fixtures_validate_and_filter() {
    use dasauto_core::sync::{filter_completed, validate_queue, SyncEntity, SyncOp};

    let gol: Vehicle =
        serde_json::from_str(include_str!("../../../fixtures/vehicle-gol.json")).unwrap();
    let expenses: Vec<Expense> =
        serde_json::from_str(include_str!("../../../fixtures/expenses-gol.json")).unwrap();

    let ops = vec![
        SyncOp::upsert(SyncEntity::Vehicle, serde_json::to_value(&gol).unwrap()),
        SyncOp::upsert(
            SyncEntity::Expense,
            serde_json::to_value(&expenses[0]).unwrap(),
        ),
        SyncOp::delete(SyncEntity::Document, expenses[0].id),
    ];

    validate_queue(&ops).unwrap();

    // mark the vehicle upsert as synced; only the remaining ops stay
    let completed = vec![(SyncEntity::Vehicle, gol.id)];
    let remaining = filter_completed(&ops, &completed);
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].entity(), SyncEntity::Expense);
    assert_eq!(remaining[1].entity(), SyncEntity::Document);
}
