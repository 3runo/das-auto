//! Two-step "add vehicle" wizard (T1-2 step 2). Step 1 = brand/model/year/
//! fuel; step 2 = plate/UF/renavam (core `plate` + `validate_vehicle`).
//! Submit creates the vehicle via the `VehicleService` trait.
//!
//! 0.8.0-alpha.3 signal API notes:
//! - A signal reached through a *field access* on a reference (`w.make`) is
//!   parsed as a method call, so calling it needs parentheses: `(w.make)()`.
//!   (A *path* like a local `let sig = w.make;` can be called as `sig()`.)
//! - `value:` takes `impl IntoAttributeValue`, and `Signal<T: Clone +
//!   IntoAttributeValue>` implements it reactively — a bare closure is NOT
//!   accepted (E0277), so we pass the copied signal directly.
//! - `.set()` takes `&mut self`; `w: &Wizard` is a shared reference, so each
//!   step copies the signals it mutates into local (Copy) bindings and the
//!   `move` (FnMut) `oninput` closures own them.

use dioxus::prelude::*;

use crate::appstate::Wizard;
use crate::i18n;
use dasauto_core::entities::FuelType;

pub fn wizard_screen(
    w: &Wizard,
    on_next: EventHandler<()>,
    on_back: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_save: EventHandler<()>,
) -> Element {
    if (w.step)() == 1 {
        wizard_step1(w, on_next, on_cancel)
    } else {
        wizard_step2(w, on_back, on_save)
    }
}

fn wizard_step1(w: &Wizard, on_next: EventHandler<()>, on_cancel: EventHandler<()>) -> Element {
    let mut make = w.make;
    let mut model = w.model;
    let mut version = w.version;
    let mut year = w.year;
    let mut fuel = w.fuel;
    let werr = w.error;
    rsx! {
        div { class: "card",
            h2 { {i18n::WIZARD_TITLE_1} }
            if let Some(err) = werr() {
                p { class: "error", {err} }
            }
            label { class: "field-label", for: "w-make", {i18n::WIZARD_MAKE} }
            input {
                id: "w-make",
                r#type: "text",
                class: "field",
                value: make,
                oninput: move |e| make.set(e.value()),
            }
            label { class: "field-label", for: "w-model", {i18n::WIZARD_MODEL} }
            input {
                id: "w-model",
                r#type: "text",
                class: "field",
                value: model,
                oninput: move |e| model.set(e.value()),
            }
            label { class: "field-label", for: "w-version", {i18n::WIZARD_VERSION} }
            input {
                id: "w-version",
                r#type: "text",
                class: "field",
                value: version,
                oninput: move |e| version.set(e.value()),
            }
            label { class: "field-label", for: "w-year", {i18n::WIZARD_YEAR} }
            input {
                id: "w-year",
                r#type: "number",
                class: "field",
                value: year,
                oninput: move |e| year.set(e.value()),
            }
            label { class: "field-label", for: "w-fuel", {i18n::WIZARD_FUEL} }
            select {
                id: "w-fuel",
                class: "field",
                value: fuel.read().as_str(),
                oninput: move |e| {
                    fuel.set(FuelType::from_str(&e.value()));
                },
                option { value: "", {i18n::WIZARD_FUEL_EMPTY} }
                option { value: "gasoline", {i18n::FUEL_GASOLINE} }
                option { value: "ethanol", {i18n::FUEL_ETHANOL} }
                option { value: "flex", {i18n::FUEL_FLEX} }
                option { value: "diesel", {i18n::FUEL_DIESEL} }
                option { value: "hybrid", {i18n::FUEL_HYBRID} }
                option { value: "electric", {i18n::FUEL_ELECTRIC} }
            }
            div { class: "btn-row",
                button { class: "secondary", onclick: move |_| on_cancel.call(()) }
                button { class: "primary", onclick: move |_| on_next.call(()) }
            }
        }
    }
}

fn wizard_step2(w: &Wizard, on_back: EventHandler<()>, on_save: EventHandler<()>) -> Element {
    let mut plate = w.plate;
    let mut uf = w.uf;
    let mut renavam = w.renavam;
    let werr = w.error;
    let busy = w.busy;
    rsx! {
        div { class: "card",
            h2 { {i18n::WIZARD_TITLE_2} }
            if let Some(err) = werr() {
                p { class: "error", {err} }
            }
            label { class: "field-label", for: "w-plate", {i18n::WIZARD_PLATE} }
            input {
                id: "w-plate",
                r#type: "text",
                class: "field",
                value: plate,
                oninput: move |e| plate.set(e.value()),
                maxlength: 7,
            }
            label { class: "field-label", for: "w-uf", {i18n::WIZARD_UF} }
            select {
                id: "w-uf",
                class: "field",
                value: uf,
                oninput: move |e| uf.set(e.value()),
                option { value: "", {i18n::WIZARD_UF} }
                for (sigla, nome) in i18n::UFS.iter() {
                    option { value: *sigla, {format!("{sigla} — {nome}")} }
                }
            }
            label { class: "field-label", for: "w-renavam", {i18n::WIZARD_RENAVAM} }
            p { class: "hint", {i18n::WIZARD_RENAVAM_HINT} }
            input {
                id: "w-renavam",
                r#type: "text",
                class: "field",
                value: renavam,
                oninput: move |e| renavam.set(e.value()),
                maxlength: 11,
            }
            div { class: "btn-row",
                button { class: "secondary", onclick: move |_| on_back.call(()) }
                if busy() {
                    p { class: "hint", {i18n::WIZARD_SAVING} }
                } else {
                    button { class: "primary", onclick: move |_| on_save.call(()) }
                }
            }
        }
    }
}
