use crate::{components::*, state::*};
use dioxus::prelude::*;
use soto_core::*;
#[component]
pub fn Server() -> Element {
    let mut cx = use_context::<AppContext>();
    let preferences = cx.preferences.read().clone();
    let modal = cx.modal;
    let state = *cx.demo_state.read();
    let state_value = match state {
        DemoState::Ready => "ready",
        DemoState::Loading => "loading",
        DemoState::Empty => "empty",
        DemoState::Error => "error",
        DemoState::Offline => "offline",
    };
    rsx! {
        div { class: "page-content",
            div { style: "height:38px" }
            Heading {
                title: "Tu servidor, a tu manera",
                subtitle: "Configura la vista previa. Las carpetas, usuarios y datos del homelab se conectarán en las siguientes etapas.",
                eyebrow: "Servidor",
            }
            div { class: "tiles",
                for (icon, title, value, text) in [
                    (
                        "server",
                        "Homelab",
                        "Sin conectar",
                        "Esta etapa sirve la interfaz. No se ha vinculado un servidor multimedia.",
                    ),
                    (
                        "local",
                        "Biblioteca local",
                        "20 ejemplos",
                        "Ningún archivo real del equipo se ha leído ni importado.",
                    ),
                    (
                        "clock",
                        "Procesamiento",
                        "Sin tareas",
                        "No se ejecutan descargas ni conversiones multimedia.",
                    ),
                ]
                {
                    section { class: "tile",
                        Icon { name: icon, size: 30 }
                        h2 { "{title}" }
                        span { class: "stat", "{value}" }
                        p { "{text}" }
                    }
                }
            }
            h2 { class: "section-title", "Preferencias de esta vista" }
            div { class: "settings-list",
                label { r#for: "pref-tv",
                    span { "Interfaz para TV" }
                    input {
                        id: "pref-tv",
                        r#type: "checkbox",
                        checked: preferences.tv,
                        onchange: move | e | cx.preferences
                                .write().tv = e.checked(),
                    }
                }
                label { r#for: "pref-images",
                    span { "Mostrar imágenes remotas de ejemplo" }
                    input {
                        id: "pref-images",
                        r#type: "checkbox",
                        checked: preferences.remote_images,
                        onchange: move | e | cx
                                .preferences.write().remote_images = e.checked(),
                    }
                }
                p {
                    "Las imágenes se solicitan directamente a Metahub. Desactiva esta opción para utilizar fondos propios sin peticiones de imágenes a terceros."
                }
                label { r#for: "demo-state",
                    span { "Estado de demostración" }
                    select {
                        id: "demo-state",
                        class: "control",
                        value: state_value,
                        onchange: move | e | cx
                                .demo_state.set(DemoState::parse(& e.value())),
                        option { value: "ready", "Contenido disponible" }
                        option { value: "loading", "Cargando" }
                        option { value: "empty", "Biblioteca vacía" }
                        option { value: "error", "Error de carga" }
                        option { value: "offline", "Servidor desconectado" }
                    }
                }
                a {
                    class: "button secondary",
                    href: Route::new(cx.route.read().origin, Page::Library)
                            .href(),
                    "Ver estado en la biblioteca"
                }
            }
            if *cx.storage_warning.read() {
                div { class: "notice", role: "status",
                    p {
                        "El navegador no permite guardar preferencias. La interfaz seguirá funcionando durante esta sesión."
                    }
                }
            }
            h2 { class: "section-title", "Bibliotecas del homelab" }
            div { class: "notice",
                h3 { "Carpetas de Local y Torrents separadas" }
                p {
                    "El acceso a carpetas se configurará desde el servidor con permisos y rutas permitidas. Esta vista previa no solicita acceso al disco."
                }
            }
            button {
                class: "button secondary",
                onclick: move |_| pending(
                    modal,
                    "Configuración del servidor pendiente",
                    "El escaneo de carpetas, las sesiones y la persistencia del homelab se implementarán en la etapa 2. Consulta el README para conocer la configuración prevista.",
                ),
                Icon { name: "local" }
                "Acerca de las carpetas"
            }
            Footer {}
        }
    }
}
#[component]
pub fn Providers() -> Element {
    let cx = use_context::<AppContext>();
    let modal = cx.modal;
    rsx! {
        div { class: "page-content",
            div { style: "height:38px" }
            Heading {
                title: "Integraciones",
                subtitle: "Catálogos, fuentes y subtítulos para tu espacio Torrents. Ninguna cuenta está conectada en esta vista previa.",
                eyebrow: "Fuentes",
            }
            div { class: "notice",
                h2 { "Real-Debrid" }
                p { "No conectado · Las credenciales se configurarán exclusivamente en el servidor." }
                button {
                    class: "button secondary",
                    onclick: move |_| pending(
                        modal,
                        "Real-Debrid pendiente",
                        "La integración se implementará en la etapa 4. Esta interfaz no recoge tokens ni los guarda en el navegador.",
                    ),
                    Icon { name: "plug" }
                    "Acerca de la conexión"
                }
            }
            div { class: "tiles",
                for (icon, title, text, stage) in [
                    (
                        "collection",
                        "Catálogo",
                        "Descubrimiento y metadatos de películas y series.",
                        "Etapa 4",
                    ),
                    (
                        "film",
                        "Subtítulos",
                        "Selección de pistas disponibles para cada reproducción.",
                        "Etapas 3 y 4",
                    ),
                    (
                        "torrent",
                        "Proveedores de torrents",
                        "Conectores independientes con validación de fuentes.",
                        "Etapas 4 y 5",
                    ),
                ]
                {
                    section { class: "tile",
                        Icon { name: icon, size: 30 }
                        h2 { "{title}" }
                        p { "{text}" }
                        span { class: "pill", "Pendiente · {stage}" }
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Devices() -> Element {
    let mut cx = use_context::<AppContext>();
    let modal = cx.modal;
    rsx! {
        div { class: "page-content",
            div { style: "height:38px" }
            Heading {
                title: "Una biblioteca, distintas pantallas",
                subtitle: "La vista TV web permite explorar el diseño con teclado. El APK y la vinculación real llegan en una etapa posterior.",
                eyebrow: "Dispositivos",
            }
            div { class: "tiles",
                section { class: "tile",
                    Icon { name: "tv", size: 30 }
                    h2 { "Vista para TV" }
                    p { "Foco visible, iconos grandes y navegación con flechas, Enter y Escape." }
                    button {
                        class: "button",
                        onclick: move |_| {
                            cx.preferences.write().tv = true;
                            navigate(Route::new(cx.route.read().origin, Page::Home));
                        },
                        "Abrir vista TV"
                    }
                }
                section { class: "tile",
                    Icon { name: "plug", size: 30 }
                    h2 { "Vincular Android TV" }
                    p {
                        "La aplicación instalable y los códigos de vinculación se implementarán en la etapa 6."
                    }
                    button {
                        class: "button secondary",
                        onclick: move |_| pending(
                            modal,
                            "Vinculación pendiente",
                            "Todavía no hay APK ni códigos de vinculación válidos. La compatibilidad de reproducción se verificará en un dispositivo Android TV real.",
                        ),
                        "Ver estado de Android TV"
                    }
                }
                section { class: "tile",
                    Icon { name: "server", size: 30 }
                    h2 { "Navegador actual" }
                    p {
                        "Las preferencias y colecciones de ejemplo se guardan localmente. No representan una sesión autenticada."
                    }
                    span { class: "pill", "Vista previa" }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Profiles() -> Element {
    let mut cx = use_context::<AppContext>();
    let selected = cx.preferences.read().profile.clone();
    let origin = cx.route.read().origin;
    rsx! {
        div { class: "page-content",
            div { style: "height:65px" }
            Heading {
                title: "¿Quién va a ver algo hoy?",
                subtitle: "Perfiles de demostración. Esta selección no inicia sesión ni cambia permisos de acceso.",
                eyebrow: "Tu espacio",
            }
            div { class: "profile-grid",
                for profile in ["Martín", "Familia", "Invitado"] {
                    button {
                        class: "profile-card",
                        "aria-pressed": selected == profile,
                        onclick: move |_| {
                            cx.preferences.write().profile = profile.into();
                            navigate(Route::new(origin, Page::Home));
                        },
                        span { class: "avatar", "{profile.chars().next().unwrap()}" }
                        strong { "{profile}" }
                    }
                }
            }
            Footer {}
        }
    }
}
#[component]
pub fn Queue() -> Element {
    let cx = use_context::<AppContext>();
    rsx! {
        div { class: "page-content",
            Tabs {}
            Heading {
                title: "Todo bajo control",
                subtitle: "Aquí aparecerán los trabajos de tu servidor: preparación, descargas y procesamiento multimedia.",
                eyebrow: "En proceso",
            }
            section { class: "state-panel",
                Icon { name: "clock", size: 46 }
                h2 { "No hay tareas en curso" }
                p {
                    "Esta primera etapa no inicia descargas. Puedes explorar el recorrido de selección de fuentes en Torrents."
                }
                a {
                    class: "button secondary",
                    href: Route::new(Origin::Torrent, Page::Library).href(),
                    "Explorar Torrents"
                }
            }
            div { class: "notice",
                p {
                    "Las tareas serán persistentes y tendrán límites de disco, concurrencia y cancelación. Esas funciones aún están pendientes."
                }
            }
            a {
                class: "text-link",
                href: Route::new(cx.route.read().origin, Page::Server).href(),
                "Ver configuración prevista"
            }
            Footer {}
        }
    }
}
