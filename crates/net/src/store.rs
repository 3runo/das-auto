//! Browser `LocalStore` backend: `localStorage` via `gloo-storage`.
//!
//! Compiled in on wasm (feature `wasm`); on other targets this module is an
//! empty stub so host tests never touch web APIs (rules.md §4).
//!
//! Implementation note: gloo-storage 0.3's typed `get`/`set` JSON-encode
//! every value and turn missing keys into errors, which does not match the
//! `LocalStore` contract. We instead go through `Storage::raw()` (the
//! `web_sys::Storage` instance): `get_item`/`set_item`/`remove_item` are
//! raw strings and Option-based — the same semantics `MemoryStore` has,
//! including junk-data tolerance (raw reads never parse) and `None` for
//! missing keys.

#[cfg(feature = "wasm")]
mod impl_ {
    use dasauto_core::store::{LocalStore, StoreError};
    use gloo_storage::{LocalStorage, Storage};

    /// `LocalStore` backed by the browser's `localStorage`.
    ///
    /// Values are stored as plain JSON strings under the `dasauto:`
    /// namespace — the same raw strings `MemoryStore` produces, so the
    /// two backends are interchangeable from the app's point of view.
    ///
    /// `localStorage` is synchronous and single-threaded; the `LocalStore`
    /// trait's `Send + Sync` requirement is satisfied trivially (the type
    /// is stateless — every call reads/writes the browser global).
    #[derive(Default)]
    pub struct GlooStore;

    /// Maps a raw `web_sys::Storage` failure to the `LocalStore` taxonomy.
    fn err(e: impl std::fmt::Debug) -> StoreError {
        StoreError::Backend(format!("localStorage: {e:?}"))
    }

    impl LocalStore for GlooStore {
        fn get_raw(&self, key: &str) -> Result<Option<String>, StoreError> {
            // `get_item` resolves to None for missing keys — no error, no parse.
            LocalStorage::raw().get_item(key).map_err(err)
        }

        fn set_raw(&self, key: &str, value: &str) -> Result<(), StoreError> {
            LocalStorage::raw().set_item(key, value).map_err(err)
        }

        fn remove_raw(&self, key: &str) -> Result<(), StoreError> {
            LocalStorage::raw().remove_item(key).map_err(err)
        }

        fn clear_all(&self) -> Result<(), StoreError> {
            // Only our namespace (`dasauto:`) — never other apps' keys.
            // Collect first: deleting while enumerating shifts the keys.
            let raw = LocalStorage::raw();
            let len = raw.length().map_err(err)?;
            let to_delete: Vec<String> = (0..len)
                .filter_map(|i| raw.key(i).map_err(err).ok().flatten())
                .filter(|k| k.starts_with("dasauto:"))
                .collect();
            for k in &to_delete {
                raw.remove_item(k).map_err(err)?;
            }
            Ok(())
        }
    }
}

#[cfg(feature = "wasm")]
pub use impl_::GlooStore;
