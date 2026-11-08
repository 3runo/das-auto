use dioxus::prelude::*;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Expenses,
    Documents,
    Parts,
    Profile,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Home,
        Tab::Expenses,
        Tab::Documents,
        Tab::Parts,
        Tab::Profile,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Home => "Início",
            Tab::Expenses => "Despesas",
            Tab::Documents => "Documentos",
            Tab::Parts => "Peças",
            Tab::Profile => "Perfil",
        }
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut active = use_signal(|| Tab::Home);
    let select = {
        let mut active = active;
        move |t: Tab| {
            active.set(t);
        }
    };

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        main {
            h1 { "Das Auto" }
            p { class: "sub", "Skeleton T1-1 — 5 abas" }
            nav { class: "tabs",
                for t in Tab::ALL {
                    TabButton { tab: t, active: active() == t, on_click: select.clone() }
                }
            }
            section { class: "content",
                match active() {
                    Tab::Home => EmptyScreen { title: "Início" },
                    Tab::Expenses => EmptyScreen { title: "Despesas" },
                    Tab::Documents => EmptyScreen { title: "Documentos" },
                    Tab::Parts => EmptyScreen { title: "Peças" },
                    Tab::Profile => EmptyScreen { title: "Perfil" },
                }
            }
        }
    }
}

#[component]
fn TabButton(tab: Tab, active: bool, on_click: EventHandler<Tab>) -> Element {
    rsx! {
        button {
            class: if active { "tab active" } else { "tab" },
            onclick: move |_| on_click.call(tab),
            {tab.label()}
        }
    }
}

#[component]
fn EmptyScreen(title: String) -> Element {
    rsx! {
        div { class: "empty",
            h2 { {title} }
            p { "Em construção — fase 1/2." }
        }
    }
}
