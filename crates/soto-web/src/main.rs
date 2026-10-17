mod browser;
mod components;
mod pages;
mod state;
use dioxus::prelude::*;
use soto_core::*;
use state::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
fn main() {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        doc.set_title("Soto · Tu biblioteca, en tu homelab");
        if let Some(html) = doc.document_element() {
            let _ = html.set_attribute("lang", "es");
        }
        if let Ok(Some(head)) = doc.query_selector("head") {
            if let Ok(link) = doc.create_element("link") {
                let _ = link.set_attribute("rel", "stylesheet");
                let _ = link.set_attribute("href", &asset!("/assets/style.css").to_string());
                let _ = head.append_child(&link);
            }
            if let Ok(icon) = doc.create_element("link") {
                let _ = icon.set_attribute("rel", "icon");
                let _ = icon.set_attribute("href", &asset!("/assets/favicon.svg").to_string());
                let _ = head.append_child(&icon);
            }
        }
    }
    dioxus::launch(App);
}
#[component]
fn App() -> Element {
    let route = use_signal(|| {
        Route::parse(
            &web_sys::window()
                .and_then(|w| w.location().hash().ok())
                .unwrap_or_default(),
        )
    });
    let preferences = use_signal(load_preferences);
    let mut storage_warning = use_signal(|| false);
    let cx = use_context_provider(|| AppContext {
        route,
        preferences,
        storage_warning,
        genre: Signal::new("Todos".into()),
        kind: Signal::new("all".into()),
        demo_state: Signal::new(DemoState::Ready),
        modal: Signal::new(None),
        media: Rc::new(demo_media()),
        sources: Rc::new(demo_sources()),
        focus_history: Rc::new(RefCell::new(HashMap::new())),
    });
    let listener_cx = cx.clone();
    let _listeners = use_hook(move || Rc::new(browser::listeners(listener_cx)));
    use_effect(move || {
        storage_warning.set(!persist(&preferences.read()));
    });
    let focus_cx = cx.clone();
    let tv_mode = use_memo(move || preferences.read().tv);
    use_effect(move || {
        let current = route.read().clone();
        browser::focus_route(&focus_cx, &current, tv_mode());
    });
    let tv = preferences.read().tv;
    let modal = cx.modal.read().clone();
    rsx! {
        div { class: if tv { "app tv" } else { "app" },
            a {
                class: "skip",
                href: "#main-content",
                onclick: move |e| {
                    e.prevent_default();
                    if let Some(el) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("page-title"))
                        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = el.focus();
                    }
                },
                "Saltar al contenido"
            }
            components::Sidebar {}
            components::Topbar {}
            main { id: "main-content", class: "workspace",
                div { class: "demo-strip",
                    span { "Vista previa · Contenido de ejemplo · Servicios sin conectar" }
                    a { href: Route::new(route.read().origin, Page::Server).href(),
                        "Configurar vista"
                    }
                }
                pages::RouteView {}
            }
            if let Some(info) = modal {
                components::Modal { info }
            }
        }
    }
}
use wasm_bindgen::JsCast;
