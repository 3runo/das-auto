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
use dasauto_core::entities::{Expense, ExpenseCategory, Vehicle};
use dasauto_core::session::Profile;
use dasauto_core::{Local, Uuid};
use dioxus::prelude::*;

use crate::appstate::{vehicle_name, Banner, ExpenseForm, Tab};
use crate::i18n;

#[component]
pub fn HomeScreen(
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
    expenses: Signal<Vec<Expense>>,
    form: ExpenseForm,
    on_expense_save: EventHandler<()>,
    on_expense_delete: EventHandler<Uuid>,
) -> Element {
    let content: Element = match active_tab() {
        Tab::Home => home_tab(&vehicles, &on_add, &on_remove),
        Tab::Expenses => {
            expenses_tab(&expenses, &form, &on_expense_save, &on_expense_delete)
        }
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
                TabButton { tab: t, active: active_tab() == t, on_click: on_tab }
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
    let on_add = *on_add;
    let on_remove = *on_remove;
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
                    VehicleCard { vehicle: v.clone(), on_remove: on_remove }
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
    let on_logout = *on_logout;
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



/// pt-BR label for an expense category (order follows ExpenseCategory::ALL).
fn cat_label(c: ExpenseCategory) -> &'static str {
    match c {
        ExpenseCategory::Fuel => i18n::EXP_CAT_FUEL,
        ExpenseCategory::Maintenance => i18n::EXP_CAT_MAINT,
        ExpenseCategory::Insurance => i18n::EXP_CAT_INSURANCE,
        ExpenseCategory::Tax => i18n::EXP_CAT_TAX,
        ExpenseCategory::Tire => i18n::EXP_CAT_TIRE,
        ExpenseCategory::Fine => i18n::EXP_CAT_FINE,
        ExpenseCategory::Other => i18n::EXP_CAT_OTHER,
    }
}
/// Expenses tab (T2-1): one always-visible card — category chip row,
/// amount/odometer/location inputs, save — plus a month-grouped list with a
/// "current month vs last" summary (plan §4.1/4.2: no wizard, no overlay).
fn expenses_tab(
    expenses: &Signal<Vec<Expense>>,
    form: &ExpenseForm,
    on_save: &EventHandler<()>,
    on_delete: &EventHandler<Uuid>,
) -> Element {
    let on_save = *on_save;
    let on_delete = *on_delete;
    let rows = expenses().to_vec();
    let summary = dasauto_core::expenses::month_summary(&rows, Local::now().date_naive());
    let groups = dasauto_core::expenses::group_by_month(&rows);
    let sum_text = if rows.is_empty() {
        String::new()
    } else {
        let delta = match summary.delta_pct {
            Some(p) if p > 0 => format!("↑ {p}% vs. mês anterior"),
            Some(p) if p < 0 => format!("↓ {}% vs. mês anterior", p.abs()),
            Some(_) => i18n::EXP_DELTA_SAME.to_string(),
            None => i18n::EXP_DELTA_NONE.to_string(),
        };
        format!(
            "{} — {}: {}  ({})",
            i18n::EXP_MONTH,
            i18n::month_label(summary.current_month.0, summary.current_month.1),
            i18n::money_str(summary.current_total_cents),
            delta,
        )
    };
    rsx! {
        section {
            h2 { {i18n::TAB_EXPENSES} }
            ExpenseFormCard { form: *form, on_save: on_save }
            if rows.is_empty() {
                div { class: "card empty",
                    p { {i18n::EXP_EMPTY_LIST} }
                }
            } else {
                div { class: "card exp-summary",
                    p { {sum_text} }
                }
                for (m, items) in groups.iter() {
                    div { class: "exp-month",
                        h3 { {i18n::month_label(m.0, m.1)} }
                        for e in items.iter() {
                            ExpenseRow { expense: e.clone(), on_delete: on_delete }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ExpenseFormCard(form: ExpenseForm, on_save: EventHandler<()>) -> Element {
    let mut category = form.category;
    let mut title = form.title;
    let mut amount = form.amount;
    let mut date = form.date;
    let mut odometer = form.odometer;
    let mut location = form.location;
    let error = form.error;
    let busy = form.busy;
    rsx! {
        div { class: "card exp-form",
            h3 { {i18n::EXP_NEW} }
            div { class: "exp-cats",
                for c in ExpenseCategory::ALL {
                    button { class: if category() == c { "chip active" } else { "chip" },
                        onclick: move |_| category.set(c),
                        {cat_label(c)} }
                }
            }
            input {
                placeholder: i18n::EXP_TITLE_FIELD,
                oninput: move |e| title.set(e.value()),
            }
            input {
                class: "num",
                placeholder: i18n::EXP_AMOUNT,
                oninput: move |e| amount.set(e.value()),
            }
            input {
                class: "num",
                placeholder: i18n::EXP_DATE,
                oninput: move |e| date.set(e.value()),
            }
            input {
                class: "num",
                placeholder: i18n::EXP_ODOMETER,
                oninput: move |e| odometer.set(e.value()),
            }
            input {
                placeholder: i18n::EXP_LOCATION,
                oninput: move |e| location.set(e.value()),
            }
            button {
                class: "primary",
                disabled: busy(),
                onclick: move |_| on_save.call(()),
                { if busy() { i18n::EXP_SAVING } else { i18n::EXP_SAVE } }
            }
            if let Some(msg) = error() {
                p { class: "error", {msg} }
            }
        }
    }
}

#[component]
fn ExpenseRow(expense: Expense, on_delete: EventHandler<Uuid>) -> Element {
    let id = expense.id;
    let cat = cat_label(expense.category);
    let amount = i18n::money_str(expense.amount_cents);
    let day = expense.occurred_on.format("%d/%m").to_string();
    rsx! {
        div { class: "exp-row",
            span { class: "exp-day", {day} }
            span { class: "exp-title", {cat} }
            if let Some(km) = expense.odometer_km {
                span { class: "exp-odo", {format!("{km} km")} }
            }
            span { class: "exp-amount", {amount} }
            button { class: "exp-del", title: "Excluir", onclick: move |_| on_delete.call(id), "✕" }
        }
    }
}
