//! Local vehicle cache (plan.md §5.6, offline-first).
//!
//! Vehicles are persisted as a JSON array under `dasauto:vehicles:<user_id>`
//! through the `LocalStore` trait. Reads are junk-tolerant: any parse error
//! yields an empty list instead of failing the app (rules.md §3). The app
//! shows the cache immediately and reconciles with the server in the
//! background; the sync engine (T2-3) layers on top of this cache.

use std::sync::Arc;

use crate::entities::Vehicle;
use crate::store::{LocalStore, StoreError};

/// Persisted vehicle cache, one namespace per user.
#[derive(Clone)]
pub struct VehicleCache {
    store: Arc<dyn LocalStore>,
}

impl VehicleCache {
    pub fn new(store: Arc<dyn LocalStore>) -> Self {
        Self { store }
    }

    /// The storage key for a user's vehicle cache.
    pub fn key(user_id: &str) -> String {
        format!("dasauto:vehicles:{user_id}")
    }

    /// Load the cached vehicles for a user; `[]` for missing or corrupt data.
    pub fn load(&self, user_id: &str) -> Vec<Vehicle> {
        let Ok(Some(raw)) = self.store.get_raw(&Self::key(user_id)) else {
            return Vec::new();
        };
        serde_json::from_str::<Vec<Vehicle>>(&raw).unwrap_or_default()
    }

    /// Persist the given vehicles (replacing the previous cache).
    pub fn save(&self, user_id: &str, vehicles: &[Vehicle]) -> Result<(), StoreError> {
        let json =
            serde_json::to_string(vehicles).map_err(|e| StoreError::Backend(format!("serialize: {e}")))?;
        self.store.set_raw(&Self::key(user_id), &json)
    }

    /// Drop a user's cache (sign-out).
    pub fn clear(&self, user_id: &str) {
        let _ = self.store.remove_raw(&Self::key(user_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::FuelType;
    use crate::store::MemoryStore;
    use crate::Utc;
    use uuid::Uuid;

    fn vehicle(id: Uuid, user: &str) -> Vehicle {
        Vehicle {
            id,
            user_id: Uuid::parse_str(user).unwrap(),
            make: "Fiat".into(),
            model: "Uno".into(),
            version: None,
            year: 2015,
            plate: "ABC1D23".into(),
            renavam: None,
            fuel_type: FuelType::Flex,
            state_uo: "SC".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    const USER_A: &str = "11111111-1111-1111-1111-111111111111";
    const USER_B: &str = "22222222-2222-2222-2222-222222222222";

    #[test]
    fn save_then_load_round_trips() {
        let cache = VehicleCache::new(Arc::new(MemoryStore::default()));
        let v = vehicle(Uuid::new_v4(), USER_A);
        cache.save(USER_A, std::slice::from_ref(&v)).unwrap();
        let loaded = cache.load(USER_A);
        assert_eq!(loaded, vec![v]);
    }

    #[test]
    fn corrupt_data_yields_empty_not_error() {
        let store = Arc::new(MemoryStore::default());
        store
            .set_raw(&VehicleCache::key(USER_A), "{ not json ]")
            .unwrap();
        let cache = VehicleCache::new(store);
        assert!(cache.load(USER_A).is_empty());
    }

    #[test]
    fn missing_data_yields_empty() {
        let cache = VehicleCache::new(Arc::new(MemoryStore::default()));
        assert!(cache.load(USER_A).is_empty());
    }

    #[test]
    fn users_are_isolated() {
        let cache = VehicleCache::new(Arc::new(MemoryStore::default()));
        cache.save(USER_A, std::slice::from_ref(&vehicle(Uuid::new_v4(), USER_A))).unwrap();
        assert!(cache.load(USER_B).is_empty());
        assert_eq!(cache.load(USER_A).len(), 1);
    }

    #[test]
    fn save_replaces_previous_cache() {
        let cache = VehicleCache::new(Arc::new(MemoryStore::default()));
        let a = vehicle(Uuid::new_v4(), USER_A);
        let b = vehicle(Uuid::new_v4(), USER_A);
        cache.save(USER_A, std::slice::from_ref(&a)).unwrap();
        cache.save(USER_A, std::slice::from_ref(&b)).unwrap();
        assert_eq!(cache.load(USER_A), vec![b]);
    }

    #[test]
    fn clear_drops_the_cache() {
        let cache = VehicleCache::new(Arc::new(MemoryStore::default()));
        cache.save(USER_A, std::slice::from_ref(&vehicle(Uuid::new_v4(), USER_A))).unwrap();
        cache.clear(USER_A);
        assert!(cache.load(USER_A).is_empty());
    }
}
