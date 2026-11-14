//! App-level state and the boot sequence (T1-3).
//!
//! `AppBackend` bundles the concrete `SupabaseClient` (auth + vehicles),
//! the on-device `LocalStore`, and the per-user `VehicleCache`. `make`
//! reads the compile-time env vars via `option_env!`, so an app built
//! without Supabase env vars still runs and shows the unconfigured banner
//! (plan.md §5.1).
//!
//! `enter_home` implements the T1-3 load sequence: paint the local cache
//! immediately, then refresh from Supabase in the background (stale-while-
//! revalidate). A session that dies mid-flight (401) drops the app back to
//! the login screen.

use std::sync::Arc;

use dioxus::prelude::*;

use dasauto_core::api::{
    AuthError, AuthService, RepoError, SignUpOutcome, VehicleService,
};
use dasauto_core::entities::{FuelType, Vehicle};
use dasauto_core::session::{Profile, Session};
use dasauto_core::store::{keys, LocalStore, MemoryStore};
use dasauto_core::vehicle_cache::VehicleCache;
use dasauto_net::SupabaseClient;

use crate::i18n;

/// PKCE verifier key (single source: `core::store::keys`; D13 namespace).
pub const PKCE_STORAGE_KEY: &str = keys::PKCE_VERIFIER;
/// Path the Google OAuth flow redirects back to (raw-code capture; T1-3).
pub const OAUTH_CALLBACK_PATH: &str = "/das-auto";

/// Concrete backend: real Supabase client + on-device stores.
///
/// `auth` and `vehicles` are the same client (GoTrue + PostgREST share the
/// session); they are separate fields so the UI binds each service by name.
pub struct AppBackend {
    pub store: Arc<dyn LocalStore>,
    pub auth: Arc<SupabaseClient>,
    pub vehicles: Arc<SupabaseClient>,
    pub cache: Arc<VehicleCache>,
    pub configured: bool,
    pub base_url: String,
    pub publishable_key: String,
}

impl AppBackend {
    /// Builds the backend from compile-time env vars (`option_env!`).
    pub fn make() -> Self {
        let store = make_store();
        let base_url = option_env!("SUPABASE_URL").unwrap_or_default().to_string();
        let publishable_key =
            option_env!("SUPABASE_PUBLISHABLE_KEY").unwrap_or_default().to_string();
        let configured = !base_url.is_empty() && !publishable_key.is_empty();
        let auth = if configured {
            Arc::new(SupabaseClient::new(
                Some(base_url.clone()),
                Some(publishable_key.clone()),
                store.clone(),
            ))
        } else {
            Arc::new(SupabaseClient::new(None, None, store.clone()))
        };
        let cache = Arc::new(VehicleCache::new(store.clone()));
        Self {
            store,
            auth: auth.clone(),
            vehicles: auth.clone(),
            cache,
            configured,
            base_url,
            publishable_key,
        }
    }
}

impl Clone for AppBackend {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            auth: self.auth.clone(),
            vehicles: self.vehicles.clone(),
            cache: self.cache.clone(),
            configured: self.configured,
            base_url: self.base_url.clone(),
            publishable_key: self.publishable_key.clone(),
        }
    }
}

impl PartialEq for AppBackend {
    fn eq(&self, other: &Self) -> bool {
        self.configured == other.configured
            && self.base_url == other.base_url
            && Arc::ptr_eq(&self.store, &other.store)
            && Arc::ptr_eq(&self.auth, &other.auth)
            && Arc::ptr_eq(&self.vehicles, &other.vehicles)
            && Arc::ptr_eq(&self.cache, &other.cache)
            && self.publishable_key == other.publishable_key
    }
}

/// `localStorage` on the web target, in-memory elsewhere (T1-2/T1-3).
#[cfg(target_arch = "wasm32")]
fn make_store() -> Arc<dyn LocalStore> {
    Arc::new(dasauto_net::store::GlooStore)
}

#[cfg(not(target_arch = "wasm32"))]
fn make_store() -> Arc<dyn LocalStore> {
    Arc::new(MemoryStore::default())
}

/// Top-level app stage (T1-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// First render: checking for a saved session (splash).
    Booting,
    /// No session: login / signup screen.
    NoAuth,
    /// Logged in: tabbed home screen.
    Home,
    /// Add-vehicle wizard overlay.
    AddVehicle,
}

/// Bottom tab bar (T1-3; T3/T4 add content).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Expenses,
    Documents,
    Parts,
    Profile,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Home,
        Tab::Expenses,
        Tab::Documents,
        Tab::Parts,
        Tab::Profile,
    ];
}

/// Non-blocking status bar (T1-3).
#[derive(Debug, Clone)]
pub enum Banner {
    /// Built without Supabase env vars — all network actions are disabled.
    Unconfigured,
    /// Background refresh failed; stale cache data stays on screen (D11).
    SyncFailed(String),
}

/// Login vs. signup mode of the auth screen (T1-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    SignIn,
    SignUp,
}

/// Add-vehicle wizard state (T1-3). Signals are created once per app
/// lifetime via `use_hook` (the dioxus 0.8 pattern for custom state).
#[derive(Debug, Clone, Copy)]
pub struct Wizard {
    pub step: Signal<u8>,
    pub make: Signal<String>,
    pub model: Signal<String>,
    pub version: Signal<String>,
    pub year: Signal<String>,
    pub fuel: Signal<FuelType>,
    pub plate: Signal<String>,
    pub uf: Signal<String>,
    pub renavam: Signal<String>,
    pub error: Signal<Option<String>>,
    pub busy: Signal<bool>,
}

impl Wizard {
    pub(crate) fn new() -> Self {
        Self {
            step: Signal::new(1),
            make: Signal::new(String::new()),
            model: Signal::new(String::new()),
            version: Signal::new(String::new()),
            year: Signal::new(String::new()),
            fuel: Signal::new(FuelType::Flex),
            plate: Signal::new(String::new()),
            uf: Signal::new(String::new()),
            renavam: Signal::new(String::new()),
            error: Signal::new(None),
            busy: Signal::new(false),
        }
    }

    pub fn reset(&mut self) {
        self.step.set(1);
        self.make.set(String::new());
        self.model.set(String::new());
        self.version.set(String::new());
        self.year.set(String::new());
        self.fuel.set(FuelType::Flex);
        self.plate.set(String::new());
        self.uf.set(String::new());
        self.renavam.set(String::new());
        self.error.set(None);
        self.busy.set(false);
    }
}

/// Stable wizard state for the app lifetime.
pub fn use_wizard() -> Wizard {
    use_hook(Wizard::new)
}

/// Enters the logged-in stage: paints the vehicle cache immediately, then
/// refreshes from Supabase in the background (T1-3 load sequence).
pub fn enter_home(
    backend: &AppBackend,
    session: Session,
    mut stage: Signal<Stage>,
    mut active_tab: Signal<Tab>,
    mut profile: Signal<Option<Profile>>,
    mut vehicles: Signal<Vec<Vehicle>>,
    mut syncing: Signal<bool>,
    mut banner: Signal<Option<Banner>>,
) {
    profile.set(Some(session.profile()));
    let user_id = session.user_id;
    let cached = backend.cache.load(&user_id);
    vehicles.set(cached);
    banner.set(None);
    active_tab.set(Tab::Home);
    stage.set(Stage::Home);

    let auth = backend.auth.clone();
    let cache = backend.cache.clone();
    spawn(async move {
        syncing.set(true);
        match auth.list().await {
            Ok(rows) => {
                if cache.save(&user_id, &rows).is_err() {
                    // Refresh succeeded; persisting the cache failed. Rare;
                    // surface it but keep the fresh rows on screen.
                    banner.set(Some(Banner::SyncFailed(
                        i18n::ERR_OFFLINE.to_string(),
                    )));
                }
                vehicles.set(rows);
            }
            Err(RepoError::NoSession) => {
                // Session expired mid-flight: drop back to login.
                stage.set(Stage::NoAuth);
            }
            Err(e) => {
                // Stale cache stays on screen (D11).
                banner.set(Some(Banner::SyncFailed(repo_error_text(&e))));
            }
        }
        syncing.set(false);
    });
}

/// Consumes a pending PKCE callback in `localStorage` (set by the OAuth
/// redirect). Returns `None` when the key is absent or the store errors.
pub fn take_pkce_callback(store: &Arc<dyn LocalStore>) -> Option<String> {
    let raw = store.get_raw(PKCE_STORAGE_KEY).ok()??;
    let value: String = serde_json::from_str(&raw).unwrap_or_default();
    let _ = store.remove_raw(PKCE_STORAGE_KEY);
    Some(value)
}

/// Maps an `AuthError` to a pt-BR user-facing string (T1-3).
pub fn auth_error_text(e: &AuthError) -> String {
    match e {
        AuthError::BadCredentials => i18n::ERR_BAD_CREDS.to_string(),
        AuthError::EmailAlreadyExists => i18n::ERR_EMAIL_EXISTS.to_string(),
        AuthError::NoSession => i18n::ERR_SESSION.to_string(),
        AuthError::Network => i18n::ERR_OFFLINE.to_string(),
        AuthError::Unconfigured => i18n::ERR_UNCONFIGURED.to_string(),
        AuthError::Server { status, message } => {
            format!("{} {}: {}", i18n::ERR_SERVER, status, message)
        }
    }
}

/// Maps a `RepoError` to a pt-BR user-facing string (T1-3).
pub fn repo_error_text(e: &RepoError) -> String {
    match e {
        RepoError::NoSession => i18n::ERR_SESSION.to_string(),
        RepoError::Network => i18n::ERR_OFFLINE.to_string(),
        RepoError::Unconfigured => i18n::ERR_UNCONFIGURED.to_string(),
        RepoError::Server { status, message } => {
            format!("{} {} · {}", i18n::ERR_SERVER, status, message)
        }
    }
}

/// Display name for a vehicle: `Make Model Version` (version optional).
pub fn vehicle_name(v: &Vehicle) -> String {
    match &v.version {
        Some(version) if !version.is_empty() => {
            format!("{} {} {}", v.make, v.model, version)
        }
        _ => format!("{} {}", v.make, v.model),
    }
}
