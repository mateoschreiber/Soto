use crate::{components::*, state::*};
use dioxus::prelude::*;
use soto_core::*;
fn current_item(cx: &AppContext) -> Option<MediaItem> {
    let r = cx.route.read();
    let id = r.id.as_deref()?;
    if sources_for(&cx.sources, id, r.origin).is_empty() {
        return None;
    }
    cx.media.iter().find(|m| m.id == id).cloned()
}
#[component]
pub fn Detail() -> Element {
    let mut cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    let Some(item) = current_item(&cx) else {
        return rsx! {
            Missing {}
        };
    };
    let saved = cx.preferences.read().is_saved(origin, &item.id);
    let save_id = item.id.clone();
    let modal = cx.modal;
    let genres = item.genres.join(", ");
    let media_sources = sources_for(&cx.sources, &item.id, origin);
    let downloaded = media_sources
        .iter()
        .any(|s| s.status == SourceStatus::Downloaded);
    rsx! {
        div { class: "page-content",
            Tabs {}
            a {
                class: "text-link",
                href: Route::new(origin, Page::Library).href(),
                Icon { name: "back", size: 20 }
                "Volver a {origin.label()}"
            }
            div { class: "detail-layout",
                Art { item: item
                            .clone() }
                section { class: "detail-copy",
                    span { class: "eyebrow", "{origin.label()} · Contenido de ejemplo" }
                    h1 { id: "page-title", tabindex: "-1", "{item.title}" }
                    Rating { value: item.rating.clone() }
                    p { class: "detail-meta", "{item.year} · {genres} · {item.duration}" }
                    p { class: "detail-description", "{item.description}" }
                    div { class: "actions",
                        if origin == Origin::Local {
                            button {
                                class: "button",
                                onclick: move |_| pending(
                                    modal,
                                    "Reproducción pendiente",
                                    "La reproducción de archivos reales llega en la etapa 3. Este título es un ejemplo y no está vinculado a un archivo de tu equipo.",
                                ),
                                Icon { name: "play" }
                                "Reproducir archivo"
                            }
                        } else {
                            a {
                                class: "button",
                                href: Route::media(origin, Page::Sources, &item.id).href(),
                                Icon { name: "play" }
                                "Seleccionar fuente"
                            }
                        }
                        button {
                            class: "button secondary",
                            "aria-pressed": saved,
                            onclick: move | _ | cx.preferences.write().toggle_saved(origin,&
                                    save_id),
                            Icon { name: if saved { "check" } else { "plus" } }
                            if saved {
                                "Guardada en colección"
                            } else {
                                "Añadir a colección"
                            }
                        }
                    }
                    div { class: "notice",
                        h3 {
                            if origin == Origin::Local {
                                "Biblioteca local de ejemplo"
                            } else if downloaded {
                                "Descargada en Torrents · Ejemplo"
                            } else {
                                "Fuentes bajo demanda · Ejemplo"
                            }
                        }
                        p {
                            if origin == Origin::Local {
                                "No hay carpetas reales conectadas. El escaneo del homelab se implementará en la etapa 2."
                            } else {
                                "Real-Debrid no está conectado. Incluso una copia descargada conserva su origen Torrents."
                            }
                        }
                    }
                }
            }
            if item.kind == "series" {
                h2 { class: "section-title", "Episodios · Temporada de ejemplo" }
                div { class: "episodes",
                    for (i, title) in [
                        "Bienvenido de nuevo",
                        "Una nueva perspectiva",
                        "Lo que queda por descubrir",
                        "Dos mundos",
                    ]
                        .iter()
                        .enumerate()
                    {
                        button {
                            class: "episode",
                            onclick: move |_| pending(
                                modal,
                                "Episodio de ejemplo",
                                "La lista muestra el recorrido visual de una serie. La indexación de episodios y su reproducción necesitan el servidor multimedia de las siguientes etapas.",
                            ),
                            span { class: "episode-number", "{i+1:02}" }
                            div {
                                strong { "{title}" }
                                small { "47 min · Ejemplo" }
                            }
                            Icon { name: "play" }
                        }
                    }
                }
            }
            h2 { class: "section-title", "Fuentes en {origin.label()}" }
            div { class: "source-list",
                for source in media_sources {
                    button {
                        class: "source-row",
                        onclick: move |_| pending(
                            modal,
                            "Fuente de ejemplo",
                            "Esta pantalla no confirma disponibilidad real. El servidor resolverá cada fuente cuando se implemente la integración.",
                        ),
                        span { class: "quality", "{source.quality}" }
                        span { class: "source-copy",
                            strong { "{source.label}" }
                            small { "{source.size}" }
                        }
                        Icon { name: "arrow" }
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Sources() -> Element {
    let cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    let Some(item) = current_item(&cx) else {
        return rsx! {
            Missing {}
        };
    };
    let modal = cx.modal;
    let sources = sources_for(&cx.sources, &item.id, origin);
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading {
                title: "Selecciona una fuente",
                subtitle: "Fuentes de demostración. La disponibilidad se comprobará en el servidor al abrir cada versión.",
                eyebrow: format!("{} · {}", item.title, origin.label()),
            }
            a {
                class: "text-link",
                href: Route::media(origin, Page::Detail, &item.id).href(),
                Icon { name: "back", size: 20 }
                "Volver a la ficha"
            }
            div { class: "detail-layout",
                Art { item: item.clone() }
                div { class: "source-list",
                    for source in sources {
                        button {
                            class: "source-row",
                            disabled: source.status ==
                                    SourceStatus::Unavailable,
                            onclick: move |_| pending(
                                modal,
                                "Integración pendiente",
                                "La conexión a Real-Debrid y la resolución de enlaces llegan en la etapa 4. No se ha enviado ninguna solicitud ni iniciado una descarga.",
                            ),
                            span { class: "quality", "{source.quality}" }
                            span { class: "source-copy",
                                strong { "{source.label}" }
                                small { "{source.size}" }
                            }
                            Icon { name: "play" }
                        }
                    }
                    div { class: "notice",
                        h3 { "¿Prefieres conservar una copia?" }
                        p {
                            "Las descargas se administrarán en Torrents. El motor directo y los límites de almacenamiento se implementarán en la etapa 5."
                        }
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
fn Missing() -> Element {
    let cx = use_context::<AppContext>();
    let origin = cx.route.read().origin;
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading {
                title: format!("Contenido no disponible en {}", origin.label()),
                subtitle: "El título no pertenece a este origen o el enlace ya no es válido.",
            }
            a {
                class: "button",
                href: Route::new(origin, Page::Library).href(),
                "Volver a la biblioteca"
            }
            Footer {}
        }
    }
}
