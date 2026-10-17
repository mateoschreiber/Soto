use dioxus::prelude::*;
use soto_core::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoState {
    #[default]
    Ready,
    Loading,
    Empty,
    Error,
    Offline,
}
impl DemoState {
    pub fn parse(s: &str) -> Self {
        match s {
            "loading" => Self::Loading,
            "empty" => Self::Empty,
            "error" => Self::Error,
            "offline" => Self::Offline,
            _ => Self::Ready,
        }
    }
}
#[derive(Clone, PartialEq)]
pub struct DialogInfo {
    pub title: String,
    pub message: String,
}
#[derive(Clone)]
pub struct AppContext {
    pub route: Signal<Route>,
    pub preferences: Signal<Preferences>,
    pub genre: Signal<String>,
    pub kind: Signal<String>,
    pub demo_state: Signal<DemoState>,
    pub modal: Signal<Option<DialogInfo>>,
    pub storage_warning: Signal<bool>,
    pub media: Rc<Vec<MediaItem>>,
    pub sources: Rc<Vec<MediaSource>>,
    pub focus_history: Rc<RefCell<HashMap<String, String>>>,
}
impl AppContext {
    pub fn available(&self) -> Vec<MediaItem> {
        filter_media(
            &self.media,
            &self.sources,
            self.route.read().origin,
            "",
            &self.genre.read(),
            &self.kind.read(),
        )
        .into_iter()
        .cloned()
        .collect()
    }
}
pub fn pending(mut modal: Signal<Option<DialogInfo>>, title: &str, message: &str) {
    modal.set(Some(DialogInfo {
        title: title.into(),
        message: message.into(),
    }));
}
pub fn navigate(route: Route) {
    if let Some(w) = web_sys::window() {
        let _ = w.location().set_hash(&route.href());
    }
}
pub fn load_preferences() -> Preferences {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("soto.preferences.v1").ok().flatten())
        .map(|s| Preferences::from_json(&s))
        .unwrap_or_default()
}
pub fn persist(p: &Preferences) -> bool {
    let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return false;
    };
    serde_json::to_string(p)
        .ok()
        .is_some_and(|value| s.set_item("soto.preferences.v1", &value).is_ok())
}
pub fn go_back(cx: &AppContext) {
    if cx.modal.read().is_some() {
        let mut modal = cx.modal;
        modal.set(None);
        return;
    }
    let route = cx.route.read().clone();
    let target = match route.page {
        Page::Sources => Route::media(
            route.origin,
            Page::Detail,
            route.id.as_deref().unwrap_or_default(),
        ),
        Page::Detail => Route::new(route.origin, Page::Library),
        _ => Route::new(route.origin, Page::Home),
    };
    navigate(target);
}
