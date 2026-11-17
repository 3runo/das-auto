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
use appstate::{use_wizard, AppBackend, PKCE_STORAGE_KEY};
use auth::*;
use dasauto_core::api::{AuthService, VehicleService};
use dasauto_core::entities::{Expense, FuelType, Vehicle};
use dasauto_core::session::Profile;
#[cfg(target_arch = "wasm32")]
use dasauto_core::store::LocalStore;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;
use dasauto_core::Uuid;
use dioxus::prelude::*;
use home::HomeScreen;
use wizard::wizard_screen;

#[cfg(target_arch = "wasm32")]
use web_sys;

#[allow(dead_code)]
const FAVICON: Asset = asset!("/assets/favicon.ico");
#[allow(dead_code)]
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::launch(App);
}

/// Parses the amount input per plan §4.1: up to 6 significant digits and up
/// to 2 decimals; ',' and '.' both accepted as the decimal separator; an
/// optional leading `R$` is ignored. Returns cents on success, or the
/// pt-BR validation message on failure.
fn parse_money(raw: &str) -> Result<i64, String> {
    let err = i18n::EXP_ERR_AMOUNT.to_string();
    let mut text = raw.trim().to_string();
    for prefix in ["R$", "$"] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            text = stripped.trim().to_string();
            break;
        }
    }
    let (int, frac) = match text.find(|c: char| c == ',' || c == '.') {
        Some(pos) => (&text[..pos], &text[pos + 1..]),
        None => (text.as_str(), ""),
    };
    let int = int.replace(' ', "");
    if int.is_empty() || !int.chars().all(|c| c.is_ascii_digit()) {
        return Err(err);
    }
    if frac.chars().any(|c| !c.is_ascii_digit() && c != ' ') {
        return Err(err);
    }
    let frac_digits: String = frac.chars().filter(|c| c.is_ascii_digit()).collect();
    if frac_digits.len() > 2 {
        return Err(err);
    }
    let padded = format!(
        "{}{:>2}",
        frac_digits,
        "00".chars().take(2 - frac_digits.len().min(2)).collect::<String>()
    )
    .trim_end()
    .to_string();
    let value = format!("{int}{padded}");
    if value.len() > 6 {
        return Err(err);
    }
    value.parse::<i64>().map_err(|_| err)
}

/// Saves a new expense (T2-1): client-generated id (D3), then the list
/// refreshes from Supabase so the new row shows up everywhere.
fn save_expense(
    app: AppBackend,
    mut form: ExpenseForm,
    profile: Signal<Option<Profile>>,
    vehicles: Signal<Vec<Vehicle>>,
    expenses: Signal<Vec<Expense>>,
    banner: Signal<Option<Banner>>,
) {
    let category = (form.category)();
    let title = form.title.read().trim().to_string();
    let amount = form.amount.read().to_string();
    let date = form.date.read().to_string();
    let odometer = form.odometer.read().trim().to_string();
    let location = form.location.read().trim().to_string();
    form.error.set(None);

    if title.is_empty() {
        form.error.set(Some(i18n::EXP_ERR_TITLE.to_string()));
        return;
    }
    let cents = match parse_money(&amount) {
        Ok(c) if c > 0 => c,
        _ => {
            form.error.set(Some(i18n::EXP_ERR_AMOUNT.to_string()));
            return;
        }
    };
    let occurred_on = match dasauto_core::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            form.error.set(Some(i18n::EXP_ERR_DATE.to_string()));
            return;
        }
    };
    let odometer_km = match odometer.as_str() {
        "" => None,
        s => match s.parse::<i32>() {
            Ok(km) if (0..=3_000_000).contains(&km) => Some(km),
            _ => {
                form.error.set(Some(i18n::EXP_ERR_ODOMETER.to_string()));
                return;
            }
        },
    };
    let user_id: Uuid = match profile.read().as_ref() {
        Some(p) => p.id.parse().unwrap_or_else(|_| Uuid::new_v4()),
        None => {
            form.error.set(Some("Sessão encerrada — entre novamente".to_string()));
            return;
        }
    };
    let vehicle_id = match vehicles.read().first() {
        Some(v) => v.id,
        None => {
            form.error.set(Some("Cadastre um veículo primeiro".to_string()));
            return;
        }
    };

    form.busy.set(true);
    let svc = app.expenses.clone();
    let id = Uuid::new_v4();
    spawn(async move {
        let now = dasauto_core::Utc::now();
        let expense = Expense {
            id,
            vehicle_id,
            user_id,
            category,
            title,
            amount_cents: cents,
            occurred_on,
            odometer_km,
            location: if location.is_empty() { None } else { Some(location) },
            notes: None,
            deleted_at: None,
            created_at: now,
            updated_at: now,
        };
        match dasauto_core::api::ExpenseService::create(svc.as_ref(), expense).await {
            Ok(_) => {
                form.reset();
                load_expenses(app, expenses, banner);
            }
            Err(e) => {
                form.busy.set(false);
                form.error.set(Some(repo_error_text(&e)));
            }
        }
    });
}

/// Deletes an expense and refreshes the list (confirm bar/toast land in T2-3;
/// this exploratory build deletes straight away).
fn delete_expense(
    app: AppBackend,
    expenses: Signal<Vec<Expense>>,
    banner: Signal<Option<Banner>>,
    id: Uuid,
) {
    let svc = app.expenses.clone();
    spawn(async move {
        if dasauto_core::api::ExpenseService::remove(svc.as_ref(), id).await.is_ok() {
            load_expenses(app, expenses, banner);
        }
    });
}

/// Fetches every expense for the signed-in user (RLS scopes it server-side);
/// non-deleted rows replace the list signal. Errors surface via the banner
/// instead of failing silently (D3).
fn load_expenses(
    app: AppBackend,
    mut expenses: Signal<Vec<Expense>>,
    mut banner: Signal<Option<Banner>>,
) {
    let svc = app.expenses.clone();
    spawn(async move {
        match dasauto_core::api::ExpenseService::list(svc.as_ref()).await {
            Ok(rows) => {
                expenses.set(rows.into_iter().filter(|e| e.deleted_at.is_none()).collect());
            }
            Err(e) => {
                banner.set(Some(Banner::SyncFailed(repo_error_text(&e))));
            }
        }
    });
}

/// (web) Pull `?code=…` off the URL + the stored PKCE verifier, clearing
/// both so a refresh can't re-exchange the code.
#[cfg(target_arch = "wasm32")]
fn take_pkce_callback(store: &Arc<dyn LocalStore>) -> Option<(String, String)> {
    let window = web_sys::window()?;
    // web-sys `Location::search()` is `Result<String, JsValue>`; unwrap the Ok
    // string, then strip the `?code=` prefix.
    let search = window.location().search().ok()?;
    let code = search.strip_prefix("?code=")?;
    if code.is_empty() {
        return None;
    }
    let verifier = store.get_raw(PKCE_STORAGE_KEY).ok()??;
    let _ = store.remove_raw(PKCE_STORAGE_KEY);
    // The `?code=…` query stays in the URL, but re-exchange is impossible: the
    // verifier was cleared above, so a refresh returns None here. (web-sys
    // `History` has no hash-only rewrite; the URL cleanup is cosmetic.)
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

    // T1-3 boot, computed synchronously: the first paint never waits on an
    // async task (the 0.8-alpha mobile runtime can leave spawned tasks
    // pending, which froze the app on the splash). Only a *persisted*
    // session needs the async restore path (splash → home); a fresh store
    // goes straight to the login screen.
    let mut stage = Signal::new(if backend().has_saved_session {
        Stage::Booting
    } else {
        Stage::NoAuth
    });
    let mut active_tab = Signal::new(Tab::Home);
    let vehicles = use_signal(Vec::new);
    let profile = use_signal(|| None::<Profile>);
    let syncing = use_signal(|| false);
    let mut banner = use_signal(|| None::<Banner>);
    let mut auth_mode = Signal::new(AuthMode::SignIn);
    let auth_email = use_signal(String::new);
    let auth_password = use_signal(String::new);
    let auth_busy = use_signal(|| false);
    let auth_error = use_signal(|| None::<String>);
    let auth_info = use_signal(|| None::<String>);
    let wiz = use_wizard();
    let remove_confirm = use_signal(|| None::<Uuid>);
    // ---- expenses (T2-1)
    let expenses = use_signal(Vec::new);
    let expense_form = use_expense_form();
    let mut booted = use_signal(|| false);

    // ---- boot -------------------------------------------------------------
    let backend_boot = backend;
    let stage_b = stage;
    let banner_b = banner;
    use_effect(move || {
        if booted() || stage_b() != Stage::Booting {
            return;
        }
        booted.set(true);
        let backend = backend_boot;
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
                        enter_home(
                            &backend(),
                            s,
                            stage_b,
                            active_tab,
                            profile,
                            vehicles,
                            syncing,
                            banner_b,
                        );
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
                    enter_home(
                        &backend(),
                        s,
                        stage_b,
                        active_tab,
                        profile,
                        vehicles,
                        syncing,
                        banner_b,
                    );
                }
                // No persisted session: go straight to the login screen
                // (previously the stage stayed Booting forever here).
                Ok(None) => {
                    stage.set(Stage::NoAuth);
                }
                Err(e) => {
                    banner.set(Some(Banner::SyncFailed(auth_error_text(&e))));
                    stage.set(Stage::NoAuth);
                }
            }
        });
    });

    // ---- expenses: refresh whenever the tab is shown ------------------------
    {
        let backend = backend;
        let stage = stage;
        let active_tab = active_tab;
        let expenses = expenses;
        let banner = banner;
        use_effect(move || {
            if stage() == Stage::Home && active_tab() == Tab::Expenses {
                load_expenses(backend(), expenses, banner);
            }
        });
    }

    // ---- auth -------------------------------------------------------------
    let do_auth = {
        let email = auth_email;
        let password = auth_password;
        let mut busy = auth_busy;
        let mut error = auth_error;
        let stage = stage;
        let banner = banner;
        let active_tab = active_tab;
        EventHandler::new(move |_: ()| {
            let email = email.read().trim().to_string();
            let password = password.read().to_string();
            spawn(async move {
                match backend().auth.sign_in_email(&email, &password).await {
                    Ok(s) => {
                        enter_home(
                            &backend(),
                            s,
                            stage,
                            active_tab,
                            profile,
                            vehicles,
                            syncing,
                            banner,
                        );
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
        let mut busy = auth_busy;
        EventHandler::new(move |_: ()| {
            let (verifier, challenge) = dasauto_net::oauth::pkce_pair();
            let origin = current_origin();
            let redirect_to = if origin.is_empty() {
                "http://localhost:5173"
            } else {
                &origin
            };
            let url =
                dasauto_net::oauth::authorize_url(&backend().base_url, &challenge, redirect_to);
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
        let mut error = auth_error;
        let mut info = auth_info;
        let mut busy = auth_busy;
        auth_mode.set(m);
        error.set(None);
        info.set(None);
        busy.set(false);
    });

    // ---- wizard -----------------------------------------------------------
    let on_next = {
        EventHandler::new(move |_: ()| {
            let mut w = wiz;
            let make = w.make.read().trim().to_string();
            let model = w.model.read().trim().to_string();
            let year_s = w.year.read().trim().to_string();
            let _fuel_type = *w.fuel.read();
            let plate_raw = w.plate.read().trim().to_string();
            let uf = w.uf.read().clone();
            let _renavam_s = w.renavam.read().trim().to_string();

            if make.is_empty()
                || model.is_empty()
                || year_s.is_empty()
                || plate_raw.is_empty()
                || uf.is_empty()
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
        let mut w = wiz;
        w.step.set(1);
        w.error.set(None);
    });

    let on_cancel = EventHandler::new(move |_: ()| {
        let mut w = wiz;
        w.reset();
        stage.set(Stage::Home);
    });

    let on_save_vehicle = {
        let mut wiz = wiz;
        let mut stage = stage;
        let mut vehicles = vehicles;
        let mut werr = wiz.error;
        let mut wbusy = wiz.busy;
        EventHandler::new(move |_: ()| {
            let w = wiz;
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

            if make.is_empty()
                || model.is_empty()
                || year_s.is_empty()
                || plate_raw.is_empty()
                || uf.is_empty()
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
            let fuel_type = FuelType::from_label(&fuel_type);
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
            let backend = backend;
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
        let mut wiz = wiz;
        let mut stage = stage;
        wiz.reset();
        stage.set(Stage::AddVehicle);
    });

    let on_add = on_add_vehicle;

    // ---- home -------------------------------------------------------------
    let on_remove_vehicle = {
        let mut remove_confirm = remove_confirm;
        EventHandler::new(move |u: Uuid| {
            remove_confirm.set(Some(u));
        })
    };

    let on_sign_out = {
        let banner = banner;
        let stage = stage;
        EventHandler::new(move |_: ()| {
            let backend = backend;
            let mut profile = profile;
            let mut vehicles = vehicles;
            let mut banner = banner;
            let mut email = auth_email;
            let mut password = auth_password;
            let mut stage = stage;
            spawn(async move {
                if let Some(uid) = profile.read().as_ref().map(|p| p.id.clone()) {
                    backend().cache.clear(&uid.to_string());
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
        let mut remove_confirm = remove_confirm;
        let mut vehicles = vehicles;
        EventHandler::new(move |_| {
            let Some(u) = *remove_confirm.read() else {
                return;
            };
            remove_confirm.set(None);
            let new_list: Vec<Vehicle> = vehicles
                .read()
                .iter()
                .filter(|v| v.id != u)
                .cloned()
                .collect();
            vehicles.set(new_list);
            let backend = backend;
            spawn(async move {
                let _ = backend().vehicles.remove(u).await;
            });
        })
    };

    let on_remove = on_remove_vehicle;
    let on_logout = on_sign_out;

    let on_tab = EventHandler::new(move |t: Tab| {
        active_tab.set(t);
    });

    let do_expense_save = EventHandler::new(move |_: ()| {
        save_expense(backend(), expense_form, profile, vehicles, expenses, banner);
    });

    let do_expense_delete = EventHandler::new(move |id: Uuid| {
        delete_expense(backend(), expenses, banner, id);
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
                AuthScreen {
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
            } else if stage() == Stage::AddVehicle {
                { wizard_screen(&wiz, on_next, on_back, on_cancel, on_save_vehicle) }
            } else if stage() == Stage::Home {
                HomeScreen {
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
                    expenses: expenses,
                    form: expense_form,
                    on_expense_save: do_expense_save,
                    on_expense_delete: do_expense_delete,
                }
            }
        }
    }
}


