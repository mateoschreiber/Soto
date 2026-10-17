use crate::{components::*, state::*};
use dioxus::prelude::*;
use soto_core::*;
#[component]
pub fn Home() -> Element {
    let mut cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    let hero_id = if origin == Origin::Local {
        "tt11280740"
    } else {
        "tt2543164"
    };
    let hero = cx.media.iter().find(|m| m.id == hero_id).cloned().unwrap();
    let items = cx.available();
    let tv = cx.preferences.read().tv;
    let count = if tv { 6 } else { 5 };
    let featured_ids = if origin == Origin::Local {
        vec![
            "tt15239678",
            "tt0816692",
            "tt15398776",
            "tt0780504",
            "tt1856101",
            "tt11280740",
        ]
    } else {
        vec![
            "tt2543164",
            "tt15239678",
            "tt1856101",
            "tt6751668",
            "tt0470752",
            "tt1877830",
        ]
    };
    let featured: Vec<_> = featured_ids
        .iter()
        .filter_map(|id| items.iter().find(|m| m.id == *id).cloned())
        .take(count)
        .collect();
    let modal = cx.modal;
    let hero_route = Route::media(origin, Page::Detail, &hero.id).href();
    let primary_route = Route::media(
        origin,
        if origin == Origin::Local {
            Page::Detail
        } else {
            Page::Sources
        },
        &hero.id,
    )
    .href();
    let hero_genres = hero.genres.join(", ");
    let hero_cast = hero.cast.join(" · ");
    let genres = [
        ("Drama", "drama", "#ad806b"),
        ("Comedia", "laugh", "#bd9649"),
        ("Acción", "action", "#758a9e"),
        ("Romance", "heart", "#b6858c"),
        ("Suspenso", "ghost", "#8b7d9c"),
        ("Ciencia ficción", "rocket", "#2b7847"),
        ("Aventura", "compass", "#9b9275"),
        ("Animación", "film", "#7d98a3"),
    ];
    rsx! {
        section { class: "hero", "aria-label": "Contenido destacado",
            Art { item: hero
                        .clone(), backdrop: true }
            div { class: "hero-copy",
                div { class: "hero-tags",
                    span { class: "pill green", "{origin.label()}" }
                    span { class: "pill", "16+ · Ejemplo" }
                }
                h1 { id: "page-title", tabindex: "-1", "{hero.title}" }
                if hero.kind == "series" && !tv {
                    h2 { "Temporada 2" }
                }
                p { class: "hero-meta", "{hero.year} · {hero.duration} · {hero_genres}" }
                p { class: "hero-description", "{hero.description}" }
                div { class: "actions",
                    a { class: "button", href: primary_route,
                        Icon { name: "play" }
                        if origin == Origin::Local {
                            "Ver episodio"
                        } else {
                            "Explorar fuentes"
                        }
                    }
                    button {
                        class: "icon-button",
                        "aria-label": "Opciones del destacado",
                        onclick: move |_| pending(
                            modal,
                            "Tu próxima sesión",
                            "Abre la ficha para guardar este título en una colección de ejemplo o consultar sus fuentes.",
                        ),
                        "···"
                    }
                    a { class: "button secondary", href: hero_route, "Ver ficha" }
                }
                if !hero.cast.is_empty() {
                    div { class: "cast",
                        span { class: "eyebrow", "Reparto" }
                        div { class: "cast-avatars",
                            for person in &hero.cast {
                                span { key: "{person}",
                                    "{person.split_whitespace().filter_map(|p|p.chars().next()).collect::<String>()}"
                                }
                            }
                        }
                        p { "{hero_cast}" }
                    }
                }
            }
            div { class: "hero-rating",
                Rating { value: hero.rating.clone() }
                small { "Valoración de ejemplo" }
            }
        }
        nav { class: "categories", "aria-label": "Géneros",
            for (genre, icon, color) in genres {
                button {
                    class: "category",
                    style: "--category-color:{color}",
                    "aria-pressed": cx.genre.read().as_str() == genre,
                    onclick: move |_| {
                        cx.genre.set(genre.into());
                        navigate(Route::new(origin, Page::Library));
                    },
                    Icon { name: icon, size: 30 }
                    span { "{genre}" }
                }
            }
        }
        div { class: "shelves",
            Shelf {
                title: "Recomendadas para ti",
                items: featured,
                origin,
                location: "recommended",
            }
            Shelf {
                title: "Tendencias en tu biblioteca",
                items: items.iter().rev()
                        .take(count).cloned().collect(),
                origin,
                location: "trending",
            }
            Shelf {
                title: "Una buena historia",
                items: items.iter().skip(5).take(count).cloned()
                        .collect(),
                origin,
                location: "stories",
            }
            Shelf {
                title: "Añadidas recientemente",
                items: items
                    .iter()
                    .skip(10.min(items.len().saturating_sub(count)))
                    .take(count)
                    .cloned()
                    .collect(),
                origin,
                location: "recent",
            }
            Footer {}
        }
    }
}
#[component]
fn Filters() -> Element {
    let mut cx = use_context::<AppContext>();
    let kind = cx.kind.read().clone();
    let genre = cx.genre.read().clone();
    rsx! {
        div { class: "toolbar",
            div { class: "filters",
                for (value, label) in [("all", "Todo"), ("movie", "Películas"), ("series", "Series")] {
                    button {
                        class: "chip",
                        "aria-pressed": kind == value,
                        onclick: move | _ | cx.kind.set(value
                                .into()),
                        "{label}"
                    }
                }
            }
            label { class: "sr-only", r#for: "genre-filter", "Filtrar por género" }
            select {
                id: "genre-filter",
                value: genre,
                onchange: move |e| cx.genre.set(e.value()),
                for g in [
                    "Todos",
                    "Drama",
                    "Comedia",
                    "Acción",
                    "Romance",
                    "Suspenso",
                    "Ciencia ficción",
                    "Aventura",
                    "Animación",
                ]
                {
                    option { value: g, "{g}" }
                }
            }
        }
    }
}
#[component]
pub fn Library() -> Element {
    let mut cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    let items = cx.available();
    let (title, subtitle) = if origin == Origin::Local {
        (
            "Tu biblioteca local",
            "Tus películas y series, reunidas en un solo lugar. Archivos de demostración.",
        )
    } else {
        (
            "Tu espacio Torrents",
            "Descubre, guarda y organiza tus fuentes online. Los archivos descargados permanecen aquí.",
        )
    };
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading { title, subtitle, eyebrow: origin
                        .label() }
            Filters {}
            p { class: "results-count", "{items.len()} títulos de ejemplo en {origin.label()}" }
            if items.is_empty() {
                ContentState { state: DemoState::Empty }
            } else {
                div { class: "poster-grid",
                    for item in items {
                        Card {
                            key: "{item.id}",
                            item,
                            origin,
                            location: "library",
                        }
                    }
                }
            }
            if cx.genre.read().as_str() != "Todos" || cx.kind.read().as_str() != "all" {
                div { class: "actions section-title",
                    button {
                        class: "button secondary",
                        onclick: move |_| {
                            cx.genre.set("Todos".into());
                            cx.kind.set("all".into());
                        },
                        "Quitar filtros"
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Search() -> Element {
    let cx = use_context::<AppContext>();
    let route = cx.route.read().clone();
    let mut query = use_signal(|| route.query.clone());
    let route_signal = cx.route;
    use_effect(move || query.set(route_signal.read().query.clone()));
    let items: Vec<MediaItem> = filter_media(
        &cx.media,
        &cx.sources,
        route.origin,
        &route.query,
        "Todos",
        "all",
    )
    .into_iter()
    .cloned()
    .collect();
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading {
                title: if route.origin == Origin::Local { "Encuentra una película en tu archivo" } else { "Encuentra tu próxima historia" },
                subtitle: if route.origin == Origin::Local { "La búsqueda se limita a Local. Los resultados son de ejemplo." } else { "Explora el catálogo de ejemplo de Torrents. Ningún proveedor está conectado." },
                eyebrow: "Búsqueda",
            }
            form {
                class: "search-field",
                onsubmit: move |e| {
                    e.prevent_default();
                    let mut next = Route::new(route_signal.read().origin, Page::Search);
                    next.query = query();
                    navigate(next);
                },
                Icon { name: "search", size: 26 }
                label { class: "sr-only", r#for: "library-search", "Título de película o serie" }
                input {
                    id: "library-search",
                    r#type: "search",
                    value: "{query}",
                    maxlength: "200",
                    placeholder: "Escribe un título…",
                    oninput: move |e| query.set(e.value()),
                }
                button { class: "button", r#type: "submit", "Buscar" }
            }
            p { class: "results-count", "{items.len()} ",if items.len()==1{"resultado"}else{"resultados"}," en {route.origin.label()}" }
            if items.is_empty() {
                section { class: "state-panel",
                    Icon { name: "search", size: 46 }
                    h2 { "No encontramos ese título en {route.origin.label()}" }
                    p { "Prueba otro nombre o cambia de pestaña para buscar en el otro origen." }
                    a {
                        class: "button secondary",
                        href: Route::new(route.origin, Page::Search).href(),
                        "Limpiar búsqueda"
                    }
                }
            } else {
                div { class: "poster-grid",
                    for item in items {
                        Card {
                            key: "{item.id}",
                            item,
                            origin: route.origin,
                            location: "search",
                        }
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Collections() -> Element {
    let cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    let preferences = cx.preferences.read();
    let items: Vec<_> = filter_media(&cx.media, &cx.sources, origin, "", "Todos", "all")
        .into_iter()
        .filter(|m| preferences.is_saved(origin, &m.id))
        .cloned()
        .collect();
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading {
                title: "Mis colecciones",
                subtitle: "Una selección para volver cuando quieras. Guardada solo en este navegador, por origen.",
                eyebrow: origin.label(),
            }
            if items.is_empty() {
                section { class: "state-panel",
                    Icon { name: "collection", size: 46 }
                    h2 { "Tu colección empieza con una historia" }
                    p {
                        "Añade títulos desde sus fichas. Los títulos de Local y Torrents se guardan por separado."
                    }
                    a {
                        class: "button",
                        href: Route::new(origin, Page::Library).href(),
                        "Explorar {origin.label()}"
                    }
                }
            } else {
                div { class: "poster-grid",
                    for item in items {
                        Card {
                            key: "{item.id}",
                            item,
                            origin,
                            location: "collection",
                        }
                    }
                }
            }
            Footer {}
        }
    }
}
