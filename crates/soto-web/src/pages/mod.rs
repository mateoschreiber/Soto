mod admin;
mod detail;
mod library;
use crate::{components::*, state::*};
use dioxus::prelude::*;
use soto_core::*;
#[component]
pub fn RouteView() -> Element {
    let cx = use_context::<AppContext>();
    let route = cx.route.read().clone();
    let state = *cx.demo_state.read();
    if state != DemoState::Ready
        && matches!(
            route.page,
            Page::Home | Page::Library | Page::Search | Page::Collections
        )
    {
        return rsx! {
            div { class: "page-content",
                Tabs {}
                Heading { title: if route.origin == Origin::Local { "Tu biblioteca local" } else { "Tu espacio Torrents" } }
                ContentState { state }
                Footer {}
            }
        };
    }
    match route.page {
        Page::Home => {
            rsx! {
                library::Home {}
            }
        }
        Page::Library => {
            rsx! {
                library::Library {}
            }
        }
        Page::Search => {
            rsx! {
                library::Search {}
            }
        }
        Page::Detail => {
            rsx! {
                detail::Detail {}
            }
        }
        Page::Sources => {
            rsx! {
                detail::Sources {}
            }
        }
        Page::Collections => {
            rsx! {
                library::Collections {}
            }
        }
        Page::Server => {
            rsx! {
                admin::Server {}
            }
        }
        Page::Providers => {
            rsx! {
                admin::Providers {}
            }
        }
        Page::Devices => {
            rsx! {
                admin::Devices {}
            }
        }
        Page::Profiles => {
            rsx! {
                admin::Profiles {}
            }
        }
        Page::Queue => {
            rsx! {
                admin::Queue {}
            }
        }
        Page::NotFound => {
            rsx! {
                div { class: "page-content",
                    Heading {
                        title: "Esta página no existe",
                        subtitle: "El enlace no es válido. Puedes volver al inicio de Soto.",
                    }
                    a {
                        class: "button",
                        href: Route::new(Origin::Local, Page::Home)
                                        .href(),
                        "Ir al inicio"
                    }
                    Footer {}
                }
            }
        }
    }
}
