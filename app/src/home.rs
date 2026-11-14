//! Home dashboard + bottom tab bar (T1-2). The Home tab lists the user's
//! vehicles (from cache, refreshed on sync); the other tabs are
//! phase-1/2 placeholders.
//!
//! 0.8.0-alpha.3 notes:
//! - `Profile` lives in `dasauto_core::session` (not `entities`).
//! - A signal is read by *calling* it: `vehicles()` (a path) works; calling
//!   `.iter()` directly on the `Signal` picks a different (signal-ref) method,
//!   so read the Vec first: `vehicles().iter()`.
//! - rsx `move` closures must be `'static`; an `&EventHandler` parameter is a
//!   non-`'static` borrow, so clone the (Copy) handler to an owned local first.
//!
//! `home_screen` needs the `#[component]` attribute: dioxus 0.7.10's rsx
//! requires the attribute macro to satisfy the `ComponentFunction` bound
//! (0.8.0-alpha.3 accepts it too).
//!
use dasauto_core::entities::Vehicle;
use dasauto_core::session::Profile;
use dasauto_core::Uuid;
use dioxus::prelude::*;

use crate::appstate::{Banner, Tab, vehicle_name};
use crate::i18n;

#[component]
pub fn home_screen(
    vehicles: Signal<Vec<Vehicle>>,
    profile: Signal<Option<Profile>>,
    syncing: Signal<bool>,
    banner: Signal<Option<Banner>>,
    active_tab: Signal<Tab>,
    mut remove_confirm: Signal<Option<Uuid>>,
    on_tab: EventHandler<Tab>,
    on_add: EventHandler<()>,
    on_remove: EventHandler<Uuid>,
    on_confirm_remove: EventHandler<()>,
    on_logout: EventHandler<()>,
) -> Element {
    let content: Element = match active_tab() {
        Tab::Home => home_tab(&vehicles, &on_add, &on_remove),
        Tab::Expenses => placeholder_tab(),
        Tab::Documents => placeholder_tab(),
        Tab::Parts => placeholder_tab(),
        Tab::Profile => profile_tab(&profile, &on_logout),
    };

    let banner_el: Element = match banner() {
        None => rsx! {},
        Some(Banner::Unconfigured) => {
            rsx! { div { class: "banner", {i18n::ERR_UNCONFIGURED} } }
        }
        Some(Banner::SyncFailed(msg)) => {
            rsx! { div { class: "banner", {msg} } }
        }
    };

    let remove_bar: Element = match remove_confirm() {
        None => rsx! {},
        Some(uid) => {
            let name = vehicles()
                .iter()
                .find(|v| v.id == uid)
                .map(vehicle_name)
                .unwrap_or_default();
            rsx! {
                div { class: "confirm-bar",
                    span { {format!("{} {name}?", i18n::HOME_REMOVE_Q)} }
                    button { class: "danger", onclick: move |_| on_confirm_remove.call(()) }
                    button { class: "secondary", onclick: move |_| remove_confirm.set(None) }
                }
            }
        }
    };

    rsx! {
        {banner_el}
        {remove_bar}
        {content}
        nav { class: "tabs",
            if syncing() {
                span { class: "syncing", {i18n::HOME_SYNCING} }
            }
            for t in Tab::ALL {
                TabButton { tab: t, active: active_tab() == t, on_click: on_tab.clone() }
            }
        }
    }
}

fn home_tab(
    vehicles: &Signal<Vec<Vehicle>>,
    on_add: &EventHandler<()>,
    on_remove: &EventHandler<Uuid>,
) -> Element {
    // Owned (Copy) handles so the 'static rsx closures can capture them.
    let on_add = on_add.clone();
    let on_remove = on_remove.clone();
    if vehicles().is_empty() {
        rsx! {
            div { class: "card empty",
                h2 { {i18n::HOME_EMPTY_TITLE} }
                p { {i18n::HOME_EMPTY_TEXT} }
                button { class: "primary", onclick: move |_| on_add.call(()) }
            }
        }
    } else {
        rsx! {
            section {
                h2 { {i18n::HOME_VEHICLES} }
                for v in vehicles().iter() {
                    VehicleCard { vehicle: v.clone(), on_remove: on_remove.clone() }
                }
                button { class: "link", onclick: move |_| on_add.call(()) }
            }
        }
    }
}

#[component]
fn VehicleCard(vehicle: Vehicle, on_remove: EventHandler<Uuid>) -> Element {
    let name = vehicle_name(&vehicle);
    let plate = vehicle.plate.clone();
    let uf = vehicle.state_uo.clone();
    let id = vehicle.id;
    rsx! {
        div { class: "card vehicle-card",
            h3 { {name} }
            p { class: "plate-badge", {format!("{plate} {uf}")} }
            button {
                class: "link danger",
                onclick: move |_| on_remove.call(id),
                {i18n::HOME_REMOVE}
            }
        }
    }
}

fn placeholder_tab() -> Element {
    rsx! {
        div { class: "card empty",
            h2 { {i18n::HOME_TAB_EMPTY} }
        }
    }
}

fn profile_tab(profile: &Signal<Option<Profile>>, on_logout: &EventHandler<()>) -> Element {
    let email = profile()
        .as_ref()
        .map(|p| p.email.clone())
        .unwrap_or_default();
    let on_logout = on_logout.clone();
    rsx! {
        div { class: "card",
            h2 { {i18n::TAB_PROFILE} }
            p { class: "hint", {email} }
            button { class: "danger", onclick: move |_| on_logout.call(()) }
        }
    }
}

#[component]
fn TabButton(tab: Tab, active: bool, on_click: EventHandler<Tab>) -> Element {
    let label = match tab {
        Tab::Home => i18n::TAB_HOME,
        Tab::Expenses => i18n::TAB_EXPENSES,
        Tab::Documents => i18n::TAB_DOCUMENTS,
        Tab::Parts => i18n::TAB_PARTS,
        Tab::Profile => i18n::TAB_PROFILE,
    };
    let t = tab;
    rsx! {
        button {
            class: if active { "tab active" } else { "tab" },
            onclick: move |_| on_click.call(t),
            {label}
        }
    }
}
