//! Login / signup screen (T1-2). Email + password via GoTrue, plus the
//! Google button (PKCE — gated on project configuration).

use dioxus::prelude::*;

use crate::appstate::AuthMode;
use crate::i18n;

#[component]
pub fn auth_screen(
    mode: Signal<AuthMode>,
    mut email: Signal<String>,
    mut password: Signal<String>,
    busy: Signal<bool>,
    error: Signal<Option<String>>,
    info: Signal<Option<String>>,
    on_submit: EventHandler<()>,
    on_switch: EventHandler<AuthMode>,
    on_google: EventHandler<()>,
) -> Element {
    let is_signin = mode() == AuthMode::SignIn;
    rsx! {
        div { class: "card auth-card",
            h2 { if is_signin { {i18n::AUTH_SIGNIN_TITLE} } else { {i18n::AUTH_SIGNUP_TITLE} } }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    on_submit.call(());
                },
                label { class: "field-label", for: "email", {i18n::AUTH_EMAIL} }
                input {
                    id: "email",
                    r#type: "email",
                    class: "field",
                    value: email,
                    oninput: move |e| email.set(e.value()),
                    autocomplete: "email",
                    placeholder: "voce@exemplo.com",
                }
                label { class: "field-label", for: "password", {i18n::AUTH_PASSWORD} }
                input {
                    id: "password",
                    r#type: "password",
                    class: "field",
                    value: password,
                    oninput: move |e| password.set(e.value()),
                    autocomplete: if is_signin { "current-password" } else { "new-password" },
                }
                if busy() {
                    p { class: "hint", {i18n::AUTH_LOADING} }
                } else {
                    button {
                        class: "primary",
                        {if is_signin { {i18n::AUTH_SIGNIN_TITLE} } else { {i18n::AUTH_SIGNUP_TITLE} } }
                    }
                }
                div { class: "divider", {i18n::AUTH_OR} }
                button {
                    class: "google",
                    disabled: busy(),
                    onclick: move |_| on_google.call(()),
                    if busy() { {i18n::AUTH_GOOGLE_LOADING} } else { {i18n::AUTH_GOOGLE} }
                }
            }
            if let Some(err) = error() {
                p { class: "error", {err} }
            }
            if let Some(info) = info() {
                p { class: "info", {info} }
            }
            button {
                class: "link",
                onclick: move |_| {
                    on_switch.call(if is_signin { AuthMode::SignUp } else { AuthMode::SignIn });
                },
                {if is_signin { {i18n::AUTH_SWITCH_TO_SIGNUP} } else { {i18n::AUTH_SWITCH_TO_SIGNIN} } }
            }
        }
    }
}
