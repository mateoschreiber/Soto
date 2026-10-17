use crate::api::Playback;
use dioxus::prelude::*;
use soto_core::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
#[derive(Clone, Default)]
pub struct UiPreferences {
    pub tv: bool,
}
#[derive(Clone, PartialEq)]
pub struct DialogInfo {
    pub title: String,
    pub message: String,
}
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone)]
pub struct AppContext {
    pub route: Signal<Route>,
    pub preferences: Signal<UiPreferences>,
    pub modal: Signal<Option<DialogInfo>>,
    pub revision: Signal<u64>,
    pub player: Signal<Option<(Playback, String)>>,
    pub focus_history: Rc<RefCell<HashMap<String, String>>>,
}
pub fn navigate(route: Route) {
    #[cfg(target_arch = "wasm32")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(&route.href());
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = route;
}
pub fn load_preferences() -> UiPreferences {
    let p = UiPreferences::default();
    #[cfg(target_arch = "wasm32")]
    let mut p = p;
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        p.tv = storage
            .get_item("soto.tv")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .or_else(|| {
                storage
                    .get_item("soto.preferences.v1")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v["tv"].as_bool())
            })
            .unwrap_or(false);
        let _ = storage.remove_item("soto.preferences.v1");
        let _ = storage.set_item("soto.tv", if p.tv { "true" } else { "false" });
    }

    p
}
pub fn persist(p: &UiPreferences) {
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item("soto.tv", if p.tv { "true" } else { "false" });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = p;
}
#[cfg(target_arch = "wasm32")]
pub fn go_back(cx: &AppContext) {
    let mut modal = cx.modal;
    if modal.read().is_some() {
        modal.set(None);
        return;
    }
    navigate(Route::new(cx.route.read().origin, Page::Home));
}
