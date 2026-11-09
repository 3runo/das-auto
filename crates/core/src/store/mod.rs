//! Local persistence layer (T1-2).
//!
//! `LocalStore` is the on-device cache trait: raw get/set/remove primitives
//! plus per-entity list/get/upsert/delete ops with default implementations
//! on top of them, so a backend only implements the raw surface.
//!
//! Backends:
//! - [`MemoryStore`] — in-memory fake; runs on every target and is the
//!   canonical test double (rules.md §4: host tests must stay green).
//! - `GlooStore` (wasm-only) — browser `localStorage`; lives in the `net`
//!   crate, which is where concrete storage impls belong (rules.md §4).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::entities::{Deadline, Document, Expense, SavedPartsSearch, Vehicle};

/// Storage failure. `InvalidData` covers corrupt JSON on the wire/disk;
/// `Backend` covers the storage engine itself (only reachable from real
/// backends such as `GlooStore`).
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("stored data failed to deserialize: {0}")]
    InvalidData(#[from] serde_json::Error),

    #[error("storage backend error: {0}")]
    Backend(String),
}

/// Key namespace (D13: `dasauto:` prefix). Mirrors plan.md §4 "Local store".
pub mod keys {
    use uuid::Uuid;

    /// All vehicles for the user (JSON array).
    pub const VEHICLES: &str = "dasauto:vehicles";
    /// The Supabase session (raw JSON; used by the net crate in T1-3).
    pub const SESSION: &str = "dasauto:session";
    /// Pending sync ops (JSON array; used by the persistent queue in T2-3).
    pub const SYNC_QUEUE: &str = "dasauto:syncqueue";

    /// Expenses are cached per vehicle.
    pub fn expenses(vehicle_id: &Uuid) -> String {
        format!("dasauto:expenses:{vehicle_id}")
    }

    pub fn documents(vehicle_id: &Uuid) -> String {
        format!("dasauto:documents:{vehicle_id}")
    }

    pub fn deadlines(vehicle_id: &Uuid) -> String {
        format!("dasauto:deadlines:{vehicle_id}")
    }

    pub fn saved_parts_searches(vehicle_id: &Uuid) -> String {
        format!("dasauto:saved_parts_searches:{vehicle_id}")
    }
}

/// Every locally stored entity exposes its primary key.
pub trait HasId {
    fn id(&self) -> Uuid;
}

impl HasId for Vehicle {
    fn id(&self) -> Uuid {
        self.id
    }
}
impl HasId for Expense {
    fn id(&self) -> Uuid {
        self.id
    }
}
impl HasId for Document {
    fn id(&self) -> Uuid {
        self.id
    }
}
impl HasId for Deadline {
    fn id(&self) -> Uuid {
        self.id
    }
}
impl HasId for SavedPartsSearch {
    fn id(&self) -> Uuid {
        self.id
    }
}

/// The on-device cache. All operations are fallible (corrupt data, backend
/// I/O). Backends must be safe to share across tasks.
pub trait LocalStore: Send + Sync {
    // ── raw primitives (the only ops a backend must implement) ──────────

    /// Reads the raw string stored under `key`, or `None` if absent.
    fn get_raw(&self, key: &str) -> Result<Option<String>, StoreError>;

    /// Writes `value` under `key`, replacing any previous value.
    fn set_raw(&self, key: &str, value: &str) -> Result<(), StoreError>;

    /// Removes `key`. A missing key is not an error.
    fn remove_raw(&self, key: &str) -> Result<(), StoreError>;

    /// Removes every key owned by this app (the `dasauto:` namespace).
    fn clear_all(&self) -> Result<(), StoreError>;

    // ── entities: vehicles (global list) ─────────────────────────────────

    fn list_vehicles(&self) -> Result<Vec<Vehicle>, StoreError> {
        read_list(self, keys::VEHICLES)
    }

    fn get_vehicle(&self, id: &Uuid) -> Result<Option<Vehicle>, StoreError> {
        find_in_list(self, keys::VEHICLES, id)
    }

    fn upsert_vehicle(&self, vehicle: &Vehicle) -> Result<(), StoreError> {
        upsert_in_list(self, keys::VEHICLES, vehicle.id, vehicle.clone())
    }

    fn delete_vehicle(&self, id: &Uuid) -> Result<(), StoreError> {
        delete_from_list::<Vehicle, _>(self, keys::VEHICLES, id)
    }

    // ── entities: expenses (per-vehicle lists) ───────────────────────────

    fn list_expenses(&self, vehicle_id: &Uuid) -> Result<Vec<Expense>, StoreError> {
        read_list(self, &keys::expenses(vehicle_id))
    }

    fn get_expense(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<Option<Expense>, StoreError> {
        find_in_list(self, &keys::expenses(vehicle_id), id)
    }

    fn upsert_expense(&self, expense: &Expense) -> Result<(), StoreError> {
        upsert_in_list(
            self,
            &keys::expenses(&expense.vehicle_id),
            expense.id,
            expense.clone(),
        )
    }

    fn delete_expense(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<(), StoreError> {
        delete_from_list::<Expense, _>(self, &keys::expenses(vehicle_id), id)
    }

    // ── entities: documents (per-vehicle lists) ──────────────────────────

    fn list_documents(&self, vehicle_id: &Uuid) -> Result<Vec<Document>, StoreError> {
        read_list(self, &keys::documents(vehicle_id))
    }

    fn get_document(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<Option<Document>, StoreError> {
        find_in_list(self, &keys::documents(vehicle_id), id)
    }

    fn upsert_document(&self, document: &Document) -> Result<(), StoreError> {
        upsert_in_list(
            self,
            &keys::documents(&document.vehicle_id),
            document.id,
            document.clone(),
        )
    }

    fn delete_document(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<(), StoreError> {
        delete_from_list::<Document, _>(self, &keys::documents(vehicle_id), id)
    }

    // ── entities: deadlines (per-vehicle lists) ──────────────────────────

    fn list_deadlines(&self, vehicle_id: &Uuid) -> Result<Vec<Deadline>, StoreError> {
        read_list(self, &keys::deadlines(vehicle_id))
    }

    fn get_deadline(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<Option<Deadline>, StoreError> {
        find_in_list(self, &keys::deadlines(vehicle_id), id)
    }

    fn upsert_deadline(&self, deadline: &Deadline) -> Result<(), StoreError> {
        upsert_in_list(
            self,
            &keys::deadlines(&deadline.vehicle_id),
            deadline.id,
            deadline.clone(),
        )
    }

    fn delete_deadline(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<(), StoreError> {
        delete_from_list::<Deadline, _>(self, &keys::deadlines(vehicle_id), id)
    }

    // ── entities: saved parts searches (per-vehicle lists) ───────────────

    fn list_saved_parts_searches(
        &self,
        vehicle_id: &Uuid,
    ) -> Result<Vec<SavedPartsSearch>, StoreError> {
        read_list(self, &keys::saved_parts_searches(vehicle_id))
    }

    fn get_saved_parts_search(
        &self,
        vehicle_id: &Uuid,
        id: &Uuid,
    ) -> Result<Option<SavedPartsSearch>, StoreError> {
        find_in_list(self, &keys::saved_parts_searches(vehicle_id), id)
    }

    fn upsert_saved_parts_search(&self, search: &SavedPartsSearch) -> Result<(), StoreError> {
        upsert_in_list(
            self,
            &keys::saved_parts_searches(&search.vehicle_id),
            search.id,
            search.clone(),
        )
    }

    fn delete_saved_parts_search(&self, vehicle_id: &Uuid, id: &Uuid) -> Result<(), StoreError> {
        delete_from_list::<SavedPartsSearch, _>(self, &keys::saved_parts_searches(vehicle_id), id)
    }
}

/// Reads the JSON array stored under `key`; a missing key yields an empty
/// list (first launch), corrupt JSON yields [`StoreError::InvalidData`].
fn read_list<T: DeserializeOwned, S: LocalStore + ?Sized>(
    store: &S,
    key: &str,
) -> Result<Vec<T>, StoreError> {
    match store.get_raw(key)? {
        Some(raw) => serde_json::from_str(&raw).map_err(StoreError::from),
        None => Ok(Vec::new()),
    }
}

fn write_list<T: Serialize, S: LocalStore + ?Sized>(
    store: &S,
    key: &str,
    items: &[T],
) -> Result<(), StoreError> {
    let json = serde_json::to_string(items)?;
    store.set_raw(key, &json)
}

fn find_in_list<T, S: LocalStore + ?Sized>(
    store: &S,
    key: &str,
    id: &Uuid,
) -> Result<Option<T>, StoreError>
where
    T: DeserializeOwned + HasId,
{
    Ok(read_list(store, key)?
        .into_iter()
        .find(|item: &T| item.id() == *id))
}

fn upsert_in_list<T, S: LocalStore + ?Sized>(
    store: &S,
    key: &str,
    id: Uuid,
    item: T,
) -> Result<(), StoreError>
where
    T: Clone + Serialize + DeserializeOwned + HasId,
{
    let mut items: Vec<T> = read_list(store, key)?;
    match items.iter_mut().find(|existing| existing.id() == id) {
        Some(slot) => *slot = item,
        None => items.push(item),
    }
    write_list(store, key, &items)
}

fn delete_from_list<T, S: LocalStore + ?Sized>(
    store: &S,
    key: &str,
    id: &Uuid,
) -> Result<(), StoreError>
where
    T: Serialize + DeserializeOwned + HasId,
{
    let mut items: Vec<T> = read_list(store, key)?;
    items.retain(|item| item.id() != *id);
    write_list(store, key, &items)
}

/// In-memory `LocalStore` — the canonical test fake (rules.md §4).
///
/// Semantics mirror a real backend: values are JSON strings in a map, so
/// deserialization failures behave exactly as they would on `localStorage`.
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl LocalStore for MemoryStore {
    fn get_raw(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.map().get(key).cloned())
    }

    fn set_raw(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.map().insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn remove_raw(&self, key: &str) -> Result<(), StoreError> {
        self.map().remove(key);
        Ok(())
    }

    fn clear_all(&self) -> Result<(), StoreError> {
        self.map().retain(|k, _| !k.starts_with("dasauto:"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{
        Deadline, DeadlineKind, DocType, Document, Expense, ExpenseCategory, FuelType,
        SavedPartsSearch, Vehicle,
    };
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    fn utc(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    fn vehicle(plate: &str, vehicle_id: Uuid) -> Vehicle {
        Vehicle {
            id: vehicle_id,
            user_id: Uuid::from_u128(1),
            make: "Volkswagen".into(),
            model: "Gol".into(),
            version: None,
            year: 2018,
            plate: plate.into(),
            renavam: None,
            fuel_type: FuelType::Flex,
            state_uo: "SP".into(),
            created_at: utc(2026, 11, 5, 12),
            updated_at: utc(2026, 11, 5, 12),
        }
    }

    fn expense(vehicle_id: Uuid, expense_id: Uuid) -> Expense {
        Expense {
            id: expense_id,
            vehicle_id,
            user_id: Uuid::from_u128(1),
            category: ExpenseCategory::Fuel,
            title: "Gasolina".into(),
            amount_cents: 25000,
            occurred_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            odometer_km: None,
            location: None,
            notes: None,
            deleted_at: None,
            created_at: utc(2026, 11, 1, 18),
            updated_at: utc(2026, 11, 1, 18),
        }
    }

    fn document(vehicle_id: Uuid, doc_id: Uuid) -> Document {
        Document {
            id: doc_id,
            vehicle_id,
            user_id: Uuid::from_u128(1),
            name: "CRLV 2025".into(),
            doc_type: DocType::Crlv,
            storage_path: "documents/x/crlv.pdf".into(),
            valid_until: None,
            valid_from: None,
            deleted_at: None,
            created_at: utc(2026, 11, 2, 10),
            updated_at: utc(2026, 11, 2, 10),
        }
    }

    fn deadline(vehicle_id: Uuid, deadline_id: Uuid) -> Deadline {
        Deadline {
            id: deadline_id,
            vehicle_id,
            user_id: Uuid::from_u128(1),
            kind: DeadlineKind::Ipva,
            label: "IPVA 2026".into(),
            due_on: NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            paid: false,
            deep_link: None,
            deleted_at: None,
            created_at: utc(2026, 11, 3, 9),
            updated_at: utc(2026, 11, 3, 9),
        }
    }

    fn saved_search(vehicle_id: Uuid, search_id: Uuid) -> SavedPartsSearch {
        SavedPartsSearch {
            id: search_id,
            vehicle_id,
            user_id: Uuid::from_u128(1),
            query: "pastilha de freio".into(),
            snapshot: serde_json::json!([{ "id": "MLB1" }]),
            created_at: utc(2026, 11, 4, 20),
        }
    }

    fn u(n: u8) -> Uuid {
        Uuid::from_u128(u128::from(n))
    }

    #[test]
    fn key_names_match_plan() {
        assert_eq!(keys::VEHICLES, "dasauto:vehicles");
        assert_eq!(keys::SESSION, "dasauto:session");
        assert_eq!(keys::SYNC_QUEUE, "dasauto:syncqueue");
        let vid = u(9);
        assert_eq!(keys::expenses(&vid), format!("dasauto:expenses:{vid}"));
        assert_eq!(keys::documents(&vid), format!("dasauto:documents:{vid}"));
        assert_eq!(keys::deadlines(&vid), format!("dasauto:deadlines:{vid}"));
        assert_eq!(
            keys::saved_parts_searches(&vid),
            format!("dasauto:saved_parts_searches:{vid}")
        );
    }

    #[test]
    fn fresh_store_is_empty() {
        let store = MemoryStore::new();
        assert!(store.list_vehicles().unwrap().is_empty());
        assert_eq!(store.get_vehicle(&u(1)).unwrap(), None);
        let vid = u(2);
        assert!(store.list_expenses(&vid).unwrap().is_empty());
        assert!(store.list_documents(&vid).unwrap().is_empty());
        assert!(store.list_deadlines(&vid).unwrap().is_empty());
        assert!(store.list_saved_parts_searches(&vid).unwrap().is_empty());
        assert_eq!(store.get_raw(keys::VEHICLES).unwrap(), None);
    }

    #[test]
    fn vehicle_crud_cycle() {
        let store = MemoryStore::new();
        let v1 = vehicle("AB1C2D3", u(1));
        let v2 = vehicle("XY9Z8W7", u(2));

        store.upsert_vehicle(&v1).unwrap();
        store.upsert_vehicle(&v2).unwrap();
        assert_eq!(store.list_vehicles().unwrap().len(), 2);
        assert_eq!(store.get_vehicle(&u(1)).unwrap(), Some(v1.clone()));

        // upsert with the same id replaces, does not duplicate
        let mut v1b = v1.clone();
        v1b.model = "Gol Rallye".into();
        store.upsert_vehicle(&v1b).unwrap();
        let all = store.list_vehicles().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            store.get_vehicle(&u(1)).unwrap().unwrap().model,
            "Gol Rallye"
        );

        store.delete_vehicle(&u(1)).unwrap();
        assert_eq!(store.get_vehicle(&u(1)).unwrap(), None);
        assert_eq!(store.list_vehicles().unwrap().len(), 1);
        // deleting a missing id is a no-op
        store.delete_vehicle(&u(99)).unwrap();
    }

    #[test]
    fn plate_is_stored_normalized() {
        let store = MemoryStore::new();
        let v = vehicle(crate::plate::normalize_plate("ab 12c34").as_str(), u(1));
        store.upsert_vehicle(&v).unwrap();
        // the raw JSON on disk is already canonical
        let raw = store.get_raw(keys::VEHICLES).unwrap().unwrap();
        assert!(raw.contains("\"AB12C34\""));
        assert!(!raw.contains("ab 12c34"));
    }

    #[test]
    fn expenses_are_scoped_per_vehicle() {
        let store = MemoryStore::new();
        let (va, vb) = (u(1), u(2));
        let ea = expense(va, u(10));
        let eb = expense(vb, u(11));
        store.upsert_expense(&ea).unwrap();
        store.upsert_expense(&eb).unwrap();

        assert_eq!(store.list_expenses(&va).unwrap(), vec![ea.clone()]);
        assert_eq!(store.list_expenses(&vb).unwrap(), vec![eb.clone()]);
        assert_eq!(store.get_expense(&va, &u(11)).unwrap(), None);
        assert_eq!(store.get_expense(&vb, &u(10)).unwrap(), None);

        // upsert replaces in place
        let mut ea2 = ea.clone();
        ea2.amount_cents = 0;
        store.upsert_expense(&ea2).unwrap();
        assert_eq!(store.list_expenses(&va).unwrap().len(), 1);
        assert_eq!(store.list_expenses(&va).unwrap()[0].amount_cents, 0);

        store.delete_expense(&va, &u(10)).unwrap();
        assert!(store.list_expenses(&va).unwrap().is_empty());
        assert_eq!(store.list_expenses(&vb).unwrap().len(), 1);
    }

    #[test]
    fn documents_crud_cycle() {
        let store = MemoryStore::new();
        let vid = u(1);
        let d = document(vid, u(20));
        store.upsert_document(&d).unwrap();
        assert_eq!(store.get_document(&vid, &u(20)).unwrap(), Some(d.clone()));
        assert_eq!(store.list_documents(&vid).unwrap().len(), 1);
        store.delete_document(&vid, &u(20)).unwrap();
        assert!(store.list_documents(&vid).unwrap().is_empty());
    }

    #[test]
    fn deadlines_crud_cycle() {
        let store = MemoryStore::new();
        let vid = u(1);
        let dl = deadline(vid, u(30));
        store.upsert_deadline(&dl).unwrap();
        assert_eq!(
            store.get_deadline(&vid, &u(30)).unwrap().unwrap().label,
            "IPVA 2026"
        );
        // upsert toggles `paid` in place
        let mut dl2 = dl.clone();
        dl2.paid = true;
        store.upsert_deadline(&dl2).unwrap();
        assert!(store.list_deadlines(&vid).unwrap().len() == 1);
        assert!(store.list_deadlines(&vid).unwrap()[0].paid);
        store.delete_deadline(&vid, &u(30)).unwrap();
        assert!(store.list_deadlines(&vid).unwrap().is_empty());
    }

    #[test]
    fn saved_searches_crud_cycle() {
        let store = MemoryStore::new();
        let vid = u(1);
        let s = saved_search(vid, u(40));
        store.upsert_saved_parts_search(&s).unwrap();
        let got = store.get_saved_parts_search(&vid, &u(40)).unwrap();
        assert_eq!(got.unwrap().query, "pastilha de freio");
        assert_eq!(store.list_saved_parts_searches(&vid).unwrap().len(), 1);
        store.delete_saved_parts_search(&vid, &u(40)).unwrap();
        assert!(store.list_saved_parts_searches(&vid).unwrap().is_empty());
    }

    #[test]
    fn corrupt_data_yields_invalid_data_error() {
        let store = MemoryStore::new();
        store.set_raw(keys::VEHICLES, "{not json").unwrap();
        let err = store.list_vehicles().unwrap_err();
        assert!(matches!(err, StoreError::InvalidData(_)));
        // the raw value is still readable — only parsing fails
        assert_eq!(
            store.get_raw(keys::VEHICLES).unwrap().as_deref(),
            Some("{not json")
        );

        // an empty-string value is corrupt too (valid for localStorage, not JSON)
        store.set_raw(keys::VEHICLES, "").unwrap();
        assert!(matches!(
            store.get_vehicle(&u(1)).unwrap_err(),
            StoreError::InvalidData(_)
        ));
    }

    #[test]
    fn deleting_the_last_item_writes_an_empty_array() {
        let store = MemoryStore::new();
        let v = vehicle("AB1C2D3", u(1));
        store.upsert_vehicle(&v).unwrap();
        store.delete_vehicle(&u(1)).unwrap();
        assert_eq!(
            store.get_raw(keys::VEHICLES).unwrap().as_deref(),
            Some("[]")
        );
        assert!(store.list_vehicles().unwrap().is_empty());
    }

    #[test]
    fn remove_raw_on_missing_key_is_ok() {
        let store = MemoryStore::new();
        store.remove_raw("dasauto:never-written").unwrap();
        assert_eq!(store.get_raw("dasauto:never-written").unwrap(), None);
    }

    #[test]
    fn clear_all_removes_only_app_namespace() {
        let store = MemoryStore::new();
        store.set_raw("dasauto:vehicles", "[]").unwrap();
        store.set_raw("dasauto:session", "{}").unwrap();
        store.set_raw("otherapp:key", "keep").unwrap();
        store.clear_all().unwrap();
        assert_eq!(store.get_raw("dasauto:vehicles").unwrap(), None);
        assert_eq!(store.get_raw("dasauto:session").unwrap(), None);
        assert_eq!(
            store.get_raw("otherapp:key").unwrap().as_deref(),
            Some("keep")
        );
    }

    fn assert_store_is_send_sync<T: LocalStore>() {}

    #[test]
    fn memory_store_is_send_and_sync() {
        assert_store_is_send_sync::<MemoryStore>();
    }
}
