//! Das Auto — network layer (plan.md §2).
//!
//! `client` implements the core service traits over Supabase REST
//! (GoTrue + PostgREST); `oauth` holds the PKCE helpers for the Google flow;
//! `store` provides the wasm `LocalStore` backend (gloo). Everything is
//! behind the core traits so the UI and tests can run on in-memory fakes
//! (rules.md §4).

pub mod client;
pub mod oauth;

#[cfg(feature = "wasm")]
pub mod store;

pub use client::SupabaseClient;

#[cfg(test)]
mod tests {
    #[test]
    fn net_crate_is_present_in_workspace() {
        assert_eq!(env!("CARGO_PKG_NAME"), "dasauto-net");
    }
}
