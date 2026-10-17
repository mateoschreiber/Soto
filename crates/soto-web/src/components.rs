use crate::state::*;
use dioxus::prelude::*;
use soto_core::*;
use wasm_bindgen::JsCast;
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    fn test_card() -> Element {
        use_context_provider(|| AppContext {
            route: Signal::new(Route::default()),
            preferences: Signal::new(Preferences {
                remote_images: false,
                ..Preferences::default()
            }),
            genre: Signal::new("Todos".into()),
            kind: Signal::new("all".into()),
            demo_state: Signal::new(DemoState::Ready),
            modal: Signal::new(None),
            storage_warning: Signal::new(false),
            media: Rc::new(demo_media()),
            sources: Rc::new(demo_sources()),
            focus_history: Rc::new(RefCell::new(HashMap::new())),
        });
        rsx! {
            Card {
                item: demo_media()[0].clone(),
                origin: Origin::Local,
                location: "test",
            }
        }
    }
    #[test]
    fn cards_render_real_metadata_and_origin_not_template_tokens() {
        let mut dom = VirtualDom::new(test_card);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(html.contains("2024 · Local"), "{html}");
        assert!(!html.contains("{item.year}"));
        assert!(html.contains("#/local/detail/tt15239678"));
    }
}
#[component]
pub fn Icon(name: String, #[props(default = 24)] size: u32) -> Element {
    let path = match name.as_str() {
        "home" => "M3 10 12 3l9 7v11h-7v-7h-4v7H3Z",
        "local" => "M3 5h6l2 3h10v13H3Z",
        "torrent" => "M12 3v13m-6-6 6 6 6-6M3 17v5h18v-5",
        "collection" => "M3 3h7v7H3ZM14 3h7v7h-7ZM3 14h7v7H3ZM14 14h7v7h-7Z",
        "clock" => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 4v6l4 2",
        "server" => "M3 3h18v7H3ZM3 14h18v7H3ZM6 6h2m-2 11h2",
        "plug" => "M8 2v6m8-6v6M5 8h14v5a7 7 0 0 1-14 0Zm7 12v3",
        "tv" => "M2 4h20v14H2Zm10 14v4m-5 0h10",
        "search" => "M10.5 2a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17ZM17 17l6 6",
        "play" => "M5 3 21 12 5 21Z",
        "back" => "M15 4 7 12l8 8M7 12h15",
        "close" => "m5 5 14 14M19 5 5 19",
        "plus" => "M12 4v16M4 12h16",
        "check" => "m4 12 5 5L21 5",
        "arrow" => "M3 12h18m-7-7 7 7-7 7",
        "heart" => "M12 21 3 12C-2 5 7-1 12 6 17-1 26 5 21 12Z",
        "star" => "m12 2 3 6 7 1-5 5 1 8-6-3-6 3 1-8-5-5 7-1Z",
        "rocket" => "M9 15 5 19m4-12 8-5 5 1-1 5-8 8-4-3Zm7-1h.1M9 7 3 8l-1 5 7-1m4 3-1 7 5-1 1-6",
        "drama" => "M3 3c5 3 13 3 18 0v9c0 6-5 10-9 11-4-1-9-5-9-11Zm3 7h3m6 0h3M8 16q4-4 8 0",
        "laugh" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM7 8h2m6 0h2M6 13h12q-6 10-12 0",
        "action" => "M3 3 21 21m-1-18L3 20M3 15l6 6m6-18 6 6M3 3h6v6m6 6h6v6",
        "ghost" => "M4 22V10a8 8 0 0 1 16 0v12l-4-3-4 3-4-3Zm4-12h1m6 0h1",
        "compass" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm5 5-3 7-7 3 3-7Z",
        "film" => "M3 3h18v18H3ZM3 8h18M3 16h18M8 3v18m8-18v18",
        "info" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 8v7m0-11h.1",
        _ => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM12 7v6m0 4h.1",
    };
    rsx! {
        svg {
            width: size,
            height: size,
            view_box: "0 0 24 24",
            fill: if name == "play" { "currentColor" } else { "none" },
            stroke: "currentColor",
            stroke_width: "1.7",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: path }
        }
    }
}
#[component]
pub fn Sidebar() -> Element {
    let cx = use_context::<AppContext>();
    let r = cx.route.read();
    let links = [
        (Page::Home, "home", "Inicio", r.origin),
        (Page::Library, "local", "Local", Origin::Local),
        (Page::Library, "torrent", "Torrents", Origin::Torrent),
        (Page::Collections, "collection", "Colecciones", r.origin),
        (Page::Queue, "clock", "En proceso", r.origin),
        (Page::Server, "server", "Servidor", r.origin),
        (Page::Providers, "plug", "Fuentes", r.origin),
        (Page::Devices, "tv", "Dispositivos", r.origin),
    ];
    rsx! {
        aside { class: "sidebar",
            a {
                class: "brand",
                href: Route::new(r.origin, Page::Home).href(),
                "aria-label": "Soto · Inicio",
                "SOTO."
            }
            nav { "aria-label": "Navegación principal",
                for (page, icon, label, origin) in links {
                    a {
                        class: "nav-link",
                        id: "nav-{origin.path()}-{page.path()}",
                        href: Route::new(origin, page).href(),
                        "aria-current": if r.page == page && r.origin == origin { "page" } else { "false" },
                        Icon { name: icon, size: 32 }
                        span { "{label}" }
                    }
                }
            }
            div { class: "rail-note", "VISTA PREVIA" }
        }
    }
}
#[component]
pub fn Topbar() -> Element {
    let mut cx = use_context::<AppContext>();
    let r = cx.route.read().clone();
    let mut query = use_signal(|| r.query.clone());
    let query_route = cx.route;
    use_effect(move || {
        query.set(query_route.read().query.clone());
    });
    let tv = cx.preferences.read().tv;
    let profile = cx.preferences.read().profile.clone();
    rsx! {
        header { class: "topbar",
            form {
                class: "global-search",
                onsubmit: move |e| {
                    e.prevent_default();
                    let mut target = Route::new(cx.route.read().origin, Page::Search);
                    target.query = query();
                    navigate(target);
                },
                Icon { name: "search", size: 26 }
                label { class: "sr-only", r#for: "global-search", "Buscar en {r.origin.label()}" }
                input {
                    id: "global-search",
                    r#type: "search",
                    maxlength: "200",
                    placeholder: "Buscar películas o series",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                }
            }
            nav { class: "tv-nav", "aria-label": "Navegación TV",
                a {
                    class: "brand",
                    href: Route::new(r.origin, Page::Home).href(),
                    "SOTO."
                }
                div { class: "tv-links",
                    for (page, label, origin) in [
                        (Page::Home, "Inicio", r.origin),
                        (Page::Library, "Local", Origin::Local),
                        (Page::Library, "Torrents", Origin::Torrent),
                        (Page::Collections, "Colecciones", r.origin),
                        (Page::Search, "Buscar", r.origin),
                    ]
                    {
                        a {
                            id: "tv-nav-{origin.path()}-{page.path()}",
                            href: Route::new(origin, page).href(),
                            "aria-current": if r.page == page && r.origin == origin { "page" } else { "false" },
                            "{label}"
                        }
                    }
                }
            }
            div { class: "top-tools",
                span { class: "demo-badge", "Demostración" }
                button {
                    class: "button secondary tv-toggle",
                    "aria-pressed": tv,
                    onclick: move |_| {
                        let value = !cx.preferences.read().tv;
                        cx.preferences.write().tv = value;
                    },
                    Icon { name: "tv", size: 20 }
                    if tv {
                        "Salir de TV"
                    } else {
                        "Vista TV"
                    }
                }
                a {
                    class: "profile-button",
                    href: Route::new(r.origin, Page::Profiles).href(),
                    "aria-label": "Cambiar perfil",
                    span { class: "avatar", "{profile.chars().next().unwrap_or('M')}" }
                }
            }
        }
    }
}
#[component]
pub fn Tabs() -> Element {
    let cx = use_context::<AppContext>();
    let r = cx.route.read().clone();
    let page = if matches!(
        r.page,
        Page::Home | Page::Library | Page::Search | Page::Collections
    ) {
        r.page
    } else {
        Page::Library
    };
    rsx! {
        nav { class: "scope-tabs", "aria-label": "Origen del contenido",
            for origin in [Origin::Local, Origin::Torrent] {
                a {
                    href: Route {
                        origin,
                        page,
                        id: None,
                        query: r.query.clone(),
                    }
                        .href(),
                    "aria-current": if origin == r.origin { "page" } else { "false" },
                    "{origin.label()}"
                }
            }
        }
    }
}
#[component]
pub fn Heading(
    title: String,
    #[props(default = String::new())] subtitle: String,
    #[props(default = String::new())] eyebrow: String,
) -> Element {
    rsx! {
        header { class: "page-heading",
            if !eyebrow.is_empty() {
                span { class: "eyebrow", "{eyebrow}" }
            }
            h1 { id: "page-title", tabindex: "-1", "{title}" }
            if !subtitle.is_empty() {
                p { "{subtitle}" }
            }
        }
    }
}
#[component]
pub fn Art(item: MediaItem, #[props(default = false)] backdrop: bool) -> Element {
    let cx = use_context::<AppContext>();
    let mut failed = use_signal(|| false);
    let remote = cx.preferences.read().remote_images && !failed();
    let src = if backdrop {
        format!(
            "https://images.metahub.space/background/medium/{}/img",
            item.id
        )
    } else {
        format!("https://images.metahub.space/poster/medium/{}/img", item.id)
    };
    rsx! {
        div { class: if backdrop { "hero-art" } else { "art" },
            div { class: "art-fallback", "aria-hidden": "true",
                small { "SOTO · CINE" }
                "{item.title}"
            }
            if remote {
                img {
                    src,
                    alt: "",
                    loading: if backdrop { "eager" } else { "lazy" },
                    decoding: "async",
                    referrerpolicy: "no-referrer",
                    onerror: move | _ |
                            failed.set(true),
                }
            }
        }
    }
}
#[component]
pub fn Rating(value: String) -> Element {
    let label = format!("Valoración de ejemplo: {value}");
    rsx! {
        span { class: "rating", "aria-label": label, "{value}" }
    }
}
#[component]
pub fn Card(item: MediaItem, origin: Origin, location: String) -> Element {
    let href = Route::media(origin, Page::Detail, &item.id).href();
    rsx! {
        a {
            class: "poster",
            id: "card-{location}-{item.id}",
            href,
            "aria-label": "{item.title} · {origin.label()}",
            Art { item: item.clone() }
            div { class: "poster-footer",
                div { class: "poster-title",
                    "{item.title}"
                    small { "{item.year} · {origin.label()}" }
                }
                Rating { value: item.rating.clone() }
            }
        }
    }
}
#[component]
pub fn Shelf(title: String, items: Vec<MediaItem>, origin: Origin, location: String) -> Element {
    rsx! {
        section { class: "shelf", "aria-label": "{title}",
            div { class: "shelf-heading",
                h2 { "{title}" }
                a {
                    class: "text-link",
                    href: Route::new(origin, Page::Library).href(),
                    "Ver todo"
                    Icon { name: "arrow", size: 17 }
                }
            }
            div { class: "poster-grid",
                for item in items {
                    Card {
                        key: "{item.id}",
                        item,
                        origin,
                        location: location.clone(),
                    }
                }
            }
        }
    }
}
#[component]
pub fn Footer() -> Element {
    let cx = use_context::<AppContext>();
    rsx! {
        footer { class: "footer",
            span { "SOTO · Tu biblioteca, en tu homelab" }
            a { href: Route::new(cx.route.read().origin, Page::Server).href(),
                "Etapa 1 · Interfaz de ejemplo"
            }
        }
    }
}
#[component]
pub fn Modal(info: DialogInfo) -> Element {
    let mut cx = use_context::<AppContext>();
    let previous = use_hook(|| {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
    });
    use_effect(|| {
        if let Some(dialog) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("soto-modal"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDialogElement>().ok())
        {
            let _ = dialog.show_modal();
        }
    });
    use_drop(move || {
        // Wait until the dialog has left the DOM and background elements are no longer inert.
        let restore = wasm_bindgen::closure::Closure::once_into_js(move || {
            if let Some(el) = previous.filter(|el| el.is_connected()) {
                let _ = el.focus();
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(restore.unchecked_ref());
        }
    });
    rsx! {
        dialog {
            id: "soto-modal",
            class: "modal",
            "aria-labelledby": "modal-title",
            oncancel: move |event| {
                event.prevent_default();
                cx.modal.set(None);
            },
            Icon { name: "info", size: 36 }
            h2 { id: "modal-title", "{info.title}" }
            p { "{info.message}" }
            div { class: "actions",
                button { class: "button", onclick: move |_| cx.modal.set(None), "Entendido" }
            }
        }
    }
}
#[component]
pub fn ContentState(state: DemoState) -> Element {
    let mut cx = use_context::<AppContext>();
    if state == DemoState::Loading {
        return rsx! {
            section {
                class: "loading-state",
                role: "status",
                "aria-label": "Cargando contenido",
                div { class: "skeleton heading" }
                div { class: "skeleton-row",
                    for i in 0..5 {
                        div { key: "{i}", class: "skeleton" }
                    }
                }
                p { "Preparando tu biblioteca…" }
                button {
                    class: "button secondary",
                    onclick: move |_| cx.demo_state.set(DemoState::Ready),
                    "Terminar demostración de carga"
                }
            }
        };
    }
    let (title, text, icon) = match state {
        DemoState::Empty => (
            "Todavía no hay contenido aquí",
            "Vuelve a la biblioteca de ejemplo o cambia los filtros.",
            "collection",
        ),
        DemoState::Offline => (
            "No podemos conectar con tu servidor",
            "Estado de desconexión de ejemplo. No hay un homelab vinculado en esta etapa.",
            "server",
        ),
        _ => (
            "No pudimos cargar el contenido",
            "Estado de error de ejemplo. Puedes volver al contenido sin conectar servicios.",
            "info",
        ),
    };
    rsx! {
        section {
            class: "state-panel",
            role: if state == DemoState::Error { "alert" } else { "status" },
            Icon { name: icon, size: 46 }
            h2 { "{title}" }
            p { "{text}" }
            button {
                class: "button",
                onclick: move |_| {
                    cx.demo_state.set(DemoState::Ready);
                    cx.genre.set("Todos".into());
                    cx.kind.set("all".into());
                },
                "Volver al contenido"
            }
        }
    }
}
