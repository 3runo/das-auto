//! Das Auto — root app (T1-2).
//!
//! Boot: PKCE callback (Google) → restored session → NoAuth. Home: cache
//! paint + background sync. All services come from `AppBackend`
//! (rules.md §4); the Google flow uses `window.location` on the web target.

mod appstate;
mod auth;
mod home;
mod i18n;
mod wizard;

use appstate::*;
use appstate::{AppBackend, use_wizard, PKCE_STORAGE_KEY};
use auth::*;
use home::home_screen;
use std::sync::Arc;
use dasauto_core::api::{
    AuthError, AuthService, RepoError, SignUpOutcome, VehicleService,
};
use dasauto_core::entities::{FuelType, Vehicle};
use dasauto_core::session::{Profile, Session};
use dasauto_core::store::{keys, LocalStore, MemoryStore};
use dasauto_core::vehicle_cache::VehicleCache;
use dasauto_core::Uuid;
use dasauto_net::SupabaseClient;
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

/// (web) Pull `?code=…` off the URL + the stored PKCE verifier, clearing
/// both so a refresh can't re-exchange the code.
#[cfg(target_arch = "wasm32")]
fn take_pkce_callback(store: &Arc<dyn LocalStore>) -> Option<(String, String)> {
    let window = web_sys::window()?;
    let code = window.location().search().strip_prefix("?code=")?;
    if code.is_empty() {
        return None;
    }
    let verifier = store.get_raw(PKCE_STORAGE_KEY).ok()??;
    let _ = store.remove_raw(PKCE_STORAGE_KEY);
    // Keep the URL clean after the exchange.
    let _ = window.history().map(|h| h.replace_state_with_hash("#"));
    Some((code.to_string(), verifier))
}

#[cfg(target_arch = "wasm32")]
fn current_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_origin() -> String {
    String::new()
}

#[component]
fn App() -> Element {
    let backend = use_memo(AppBackend::make);

    let mut stage = Signal::new(Stage::Booting);
    let mut active_tab = Signal::new(Tab::Home);
    let mut vehicles = use_signal(Vec::new);
    let mut profile = use_signal(|| None::<Profile>);
    let mut syncing = use_signal(|| false);
    let mut banner = use_signal(|| None::<Banner>);
    let mut auth_mode = Signal::new(AuthMode::SignIn);
    let auth_email = use_signal(String::new);
    let auth_password = use_signal(String::new);
    let mut auth_busy = use_signal(|| false);
    let mut auth_error = use_signal(|| None::<String>);
    let mut auth_info = use_signal(|| None::<String>);
    let wiz = use_wizard();
    let remove_confirm = use_signal(|| None::<Uuid>);
    let mut booted = use_signal(|| false);

    // ---- boot -------------------------------------------------------------
    let backend_boot = backend.clone();
    let stage_b = stage;
    let banner_b = banner;
    use_effect(move || {
        if booted() {
            return;
        }
        booted.set(true);
        let backend = backend_boot.clone();
        let profile = profile;
        let vehicles = vehicles;
        let syncing = syncing;
        let active_tab = active_tab;
        spawn(async move {
            // 1) Google PKCE callback (web redirect lands here).
            #[cfg(target_arch = "wasm32")]
            if let Some((code, verifier)) = take_pkce_callback(&backend().store) {
                match backend().auth.exchange_pkce(&code, &verifier).await {
                    Ok(s) => {
                        enter_home(&backend(), s, stage_b, active_tab, profile, vehicles, syncing, banner_b);
                    }
                    Err(e) => {
                        banner.set(Some(Banner::SyncFailed(auth_error_text(&e))));
                        stage.set(Stage::NoAuth);
                    }
                }
            }
            // 2) Persisted session.
            match backend().auth.restore_session().await {
                Ok(Some(s)) => {
                    enter_home(&backend(), s, stage_b, active_tab, profile, vehicles, syncing, banner_b);
                }
                Ok(None) => {}
                Err(e) => {
                    banner.set(Some(Banner::SyncFailed(auth_error_text(&e))));
                    stage.set(Stage::NoAuth);
                }
            }
        });
    });

    // ---- auth -------------------------------------------------------------
    let do_auth = {
        let backend = backend.clone();
        let email = auth_email.clone();
        let password = auth_password.clone();
        let mut busy = auth_busy.clone();
        let mut error = auth_error.clone();
        let mut stage = stage;
        let mut profile = profile;
        let mut vehicles = vehicles;
        let mut syncing = syncing;
        let mut banner = banner;
        let active_tab = active_tab;
        EventHandler::new(move |_: ()| {
            let email = email.read().trim().to_string();
            let password = password.read().to_string();
            spawn(async move {
                match backend().auth.sign_in_email(&email, &password).await {
                    Ok(s) => {
                        enter_home(&backend(), s, stage, active_tab, profile, vehicles, syncing, banner);
                    }
                    Err(e) => {
                        error.set(Some(auth_error_text(&e)));
                    }
                }
                busy.set(false);
            });
        })
    };

    let do_google = {
        let backend = backend.clone();
        let mut busy = auth_busy.clone();
        let mut stage = stage;
        let mut profile = profile;
        let mut vehicles = vehicles;
        let mut syncing = syncing;
        let mut banner = banner;
        let active_tab = active_tab;
        EventHandler::new(move |_: ()| {
            let backend = backend.clone();
            let (verifier, challenge) = dasauto_net::oauth::pkce_pair();
            let origin = current_origin();
            let redirect_to = if origin.is_empty() {
                "http://localhost:5173"
            } else {
                &origin
            };
            let url = dasauto_net::oauth::authorize_url(&backend().base_url, &challenge, &redirect_to);
            let _ = backend().store.set_raw(PKCE_STORAGE_KEY, &verifier);
            #[cfg(target_arch = "wasm32")]
            if let Some(window) = web_sys::window() {
                let _ = window.location().set_href(&url);
            }
            busy.set(true);
            drop(url);
        })
    };

    let do_switch = EventHandler::new(move |m: AuthMode| {
        let mut error = auth_error.clone();
        let mut info = auth_info.clone();
        let mut busy = auth_busy.clone();
        auth_mode.set(m);
        error.set(None);
        info.set(None);
        busy.set(false);
    });

    // ---- wizard -----------------------------------------------------------
    let on_next = {
        let wiz = wiz.clone();
        EventHandler::new(move |_: ()| {
            let mut w = wiz.clone();
            let make = w.make.read().trim().to_string();
            let model = w.model.read().trim().to_string();
            let year_s = w.year.read().trim().to_string();
            let fuel_type = w.fuel.read().clone();
            let plate_raw = w.plate.read().trim().to_string();
            let uf = w.uf.read().clone();
            let _renavam_s = w.renavam.read().trim().to_string();

            if make.is_empty() || model.is_empty() || year_s.is_empty()
                || plate_raw.is_empty() || uf.is_empty()
            {
                w.error.set(Some(i18n::ERR_FILL_FIELDS.to_string()));
                return;
            }
            match year_s.parse::<i32>() {
                Ok(y) if (1900..=2100).contains(&y) => {
                    w.error.set(None);
                    w.step.set(2);
                }
                _ => w.error.set(Some(i18n::ERR_YEAR.to_string())),
            }
        })
    };

    let on_back = EventHandler::new(move |_: ()| {
        let mut w = wiz.clone();
        w.step.set(1);
        w.error.set(None);
    });

    let on_cancel = EventHandler::new(move |_: ()| {
        let mut w = wiz.clone();
        w.reset();
        stage.set(Stage::Home);
    });

    let on_save_vehicle = {
        let backend = backend.clone();
        let mut wiz = wiz.clone();
        let mut stage = stage;
        let mut vehicles = vehicles;
        let profile = profile;
        let mut werr = wiz.error.clone();
        let mut wbusy = wiz.busy.clone();
        EventHandler::new(move |_: ()| {
            let mut w = wiz.clone();
            let uid = match profile.read().as_ref() {
                Some(p) => p.id.clone(),
                None => return,
            };
            let uid: Uuid = uid.parse().unwrap_or_else(|_| Uuid::new_v4());
            let make = w.make.read().trim().to_string();
            let model = w.model.read().trim().to_string();
            let version = w.version.read().trim().to_string();
            let year_s = w.year.read().trim().to_string();
            let fuel_type = w.fuel.read().as_str().to_string();
            let plate_raw = w.plate.read().trim().to_string();
            let uf = w.uf.read().clone();
            let renavam_s = w.renavam.read().trim().to_string();

            if make.is_empty() || model.is_empty() || year_s.is_empty()
                || plate_raw.is_empty() || uf.is_empty()
            {
                werr.set(Some(i18n::ERR_FILL_FIELDS.to_string()));
                return;
            }
            let year: i32 = match year_s.parse() {
                Ok(y) if (1900..=2100).contains(&y) => y,
                _ => {
                    werr.set(Some(i18n::ERR_YEAR.to_string()));
                    return;
                }
            };
            let plate = dasauto_core::plate::normalize_plate(&plate_raw);
            if let Err(e) = dasauto_core::plate::validate_plate(&plate) {
                werr.set(Some(e.to_string()));
                return;
            }
            let fuel_type = FuelType::from_str(&fuel_type);
            let renavam = match renavam_s.as_str() {
                "" => None,
                digits => {
                    if digits.len() != 11 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        werr.set(Some(i18n::ERR_RENAVAM.to_string()));
                        return;
                    }
                    Some(digits.to_string())
                }
            };

            let now = dasauto_core::Utc::now();
            let vehicle = Vehicle {
                id: Uuid::new_v4(),
                user_id: uid,
                make,
                model,
                version: Some(version),
                year,
                fuel_type,
                plate,
                state_uo: uf,
                renavam,
                created_at: now,
                updated_at: now,
            };
            werr.set(None);
            wbusy.set(true);
            let backend = backend.clone();
            let cache = backend().cache.clone();
            spawn(async move {
                match backend().vehicles.create(vehicle.clone()).await {
                    Ok(v) => {
                        let mut all = vehicles.read().clone();
                        all.push(v);
                        vehicles.set(all.clone());
                        let _ = cache.save(&uid.to_string(), &all);
                        wiz.reset();
                        stage.set(Stage::Home);
                    }
                    Err(e) => {
                        werr.set(Some(repo_error_text(&e)));
                    }
                }
                wbusy.set(false);
            });
        })
    };

    let on_add_vehicle = EventHandler::new(move |_: ()| {
        let mut wiz = wiz.clone();
        let mut stage = stage;
        wiz.reset();
        stage.set(Stage::AddVehicle);
    });

    let on_add = on_add_vehicle;

    // ---- home -------------------------------------------------------------
    let on_remove_vehicle = {
        let backend = backend.clone();
        let mut stage = stage;
        let mut profile = profile;
        let mut vehicles = vehicles;
        let mut banner = banner;
        let remove_confirm = remove_confirm.clone();
        let email = auth_email.clone();
        let password = auth_password.clone();
        EventHandler::new(move |u: Uuid| {
            let backend = backend.clone();
            let mut stage = stage;
            let mut profile = profile;
            let mut vehicles = vehicles;
            let mut banner = banner;
            let _remove_confirm = remove_confirm;
            let mut email = auth_email;
            let mut password = auth_password;
            spawn(async move {
                let uid = u;
                let _ = backend().cache.clone();
                let _ = backend().vehicles.remove(uid).await;
                let _ = backend().cache.clear(&uid.to_string());
                let _ = backend().auth.sign_out().await;
                vehicles.set(Vec::new());
                profile.set(None);
                banner.set(None);
                email.set(String::new());
                password.set(String::new());
                stage.set(Stage::NoAuth);
                active_tab.set(Tab::Home);
            });
        })
    };

    let on_sign_out = {
        let backend = backend.clone();
        let mut profile = profile.clone();
        let mut vehicles = vehicles.clone();
        let mut banner = banner.clone();
        let mut email = auth_email.clone();
        let mut password = auth_password.clone();
        let mut stage = stage.clone();
        EventHandler::new(move |_: ()| {
            let backend = backend.clone();
            let mut profile = profile;
            let mut vehicles = vehicles;
            let mut banner = banner;
            let mut email = auth_email;
            let mut password = auth_password;
            let mut stage = stage;
            spawn(async move {
                if let Some(uid) = profile.read().as_ref().map(|p| p.id.clone()) {
                    let _ = backend().cache.clear(&uid.to_string());
                    let _ = backend().auth.sign_out().await;
                }
                vehicles.set(Vec::new());
                profile.set(None);
                banner.set(None);
                email.set(String::new());
                password.set(String::new());
                stage.set(Stage::NoAuth);
            });
        })
    };

    let on_remove_confirm = {
        let backend = backend.clone();
        let remove_confirm = remove_confirm.clone();
        let on_remove = on_remove_vehicle.clone();
        EventHandler::new(move |_| {
            if let Some(u) = remove_confirm.read().clone() {
                let _ = backend().vehicles.remove(u);
            }
        })
    };

    let on_remove = on_remove_vehicle.clone();
    let on_logout = on_sign_out.clone();

    let on_tab = EventHandler::new(move |t: Tab| {
        active_tab.set(t);
    });

    rsx! {
        div { class: "card",
            h2 { "Das Auto" }
            p { class: "sub", "Registro e diagnóstico veicular" }
            if stage() == Stage::Booting {
                p { class: "boot", "Inicializando…" }
            }
            if stage() == Stage::NoAuth {
                p { class: "sub", "Sessão encerrada — identifique-se para ver os veículos" }
                auth_screen {
                    mode: auth_mode,
                    email: auth_email,
                    password: auth_password,
                    busy: auth_busy,
                    error: auth_error,
                    info: auth_info,
                    on_submit: do_auth,
                    on_switch: do_switch,
                    on_google: do_google,
                }
            } else {
                home_screen {
                    vehicles: vehicles,
                    profile: profile,
                    remove_confirm: remove_confirm,
                    syncing: syncing,
                    banner: banner,
                    active_tab: active_tab,
                    on_tab: on_tab,
                    on_add: on_add,
                    on_remove: on_remove,
                    on_confirm_remove: on_remove_confirm,
                    on_logout: on_logout,
                }
            }
        }
    }
}

/// T1-3 load sequence: paint the local cache immediately, then refresh from
/// Supabase in the background (stale-while-revalidate).
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
    syncing.set(true);
    let backend = backend.clone();
    let mut vehicles = vehicles;
    spawn(async move {
        match backend.vehicles.list().await {
            Ok(all) => {
                vehicles.set(all);
                syncing.set(false);
            }
            Err(e) => {
                syncing.set(false);
                banner.set(Some(Banner::SyncFailed(repo_error_text(&e))));
            }
        }
    });
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

/// Consumes a pending PKCE callback in `localStorage` (set by the OAuth
/// redirect). Returns `None` when the key is absent or the store errors.
pub fn take_pkce_callback(store: &Arc<dyn LocalStore>) -> Option<String> {
    let raw = store.get_raw(PKCE_STORAGE_KEY).ok()??;
    let value: String = serde_json::from_str(&raw).unwrap_or_default();
    let _ = store.remove_raw(PKCE_STORAGE_KEY);
    Some(value)
}