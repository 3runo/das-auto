//! Das Auto — network layer (Supabase client, auth, sync).
//!
//! Everything here will sit behind traits with in-memory fakes so tests run
//! on the host and the UI can be developed offline. The real Supabase
//! client lands in T1-3.

#[cfg(feature = "wasm")]
pub mod store;

#[cfg(test)]
mod tests {
    #[test]
    fn net_crate_is_present_in_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "dasauto-net");
    }
}
