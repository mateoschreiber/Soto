use crate::state::{AppContext, go_back};
use dioxus::prelude::{ReadableExt, WritableExt};
use gloo_events::EventListener;
use soto_core::{Direction, FocusBox, Route, nearest_in_direction};
use wasm_bindgen::{JsCast, closure::Closure};
pub fn listeners(cx: AppContext) -> Vec<EventListener> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let mut hash_cx = cx.clone();
    let hashes = EventListener::new(&window, "hashchange", move |_| {
        if let Some(doc) = web_sys::window().and_then(|w| w.document())
            && let Some(el) = doc.active_element()
        {
            let id = el.id();
            if !id.is_empty() {
                hash_cx
                    .focus_history
                    .borrow_mut()
                    .insert(hash_cx.route.read().href(), id);
            }
        }
        let hash = web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default();
        hash_cx.route.set(Route::parse(&hash));
        hash_cx.modal.set(None);
    });
    let keys = EventListener::new(&window, "keydown", move |event| {
        let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let key = event.key();
        if key == "Escape" && cx.modal.read().is_some() {
            event.prevent_default();
            go_back(&cx);
            return;
        }
        if !cx.preferences.read().tv || event.ctrl_key() || event.alt_key() || event.meta_key() {
            return;
        }
        let active = document.active_element();
        if active
            .as_ref()
            .is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
        {
            return;
        }
        if key == "Escape" || key == "BrowserBack" {
            event.prevent_default();
            go_back(&cx);
            return;
        }
        let direction = match key.as_str() {
            "ArrowLeft" => Direction::Left,
            "ArrowRight" => Direction::Right,
            "ArrowUp" => Direction::Up,
            "ArrowDown" => Direction::Down,
            _ => return,
        };
        let selector = "a[href],button:not(:disabled),input,select,[tabindex='0']";
        let nodes = if let Ok(Some(dialog)) = document.query_selector("dialog[open]") {
            dialog.query_selector_all(selector)
        } else {
            document.query_selector_all(selector)
        };
        let Ok(nodes) = nodes else { return };
        let targets: Vec<web_sys::HtmlElement> = (0..nodes.length())
            .filter_map(|i| nodes.get(i)?.dyn_into().ok())
            .filter(|el: &web_sys::HtmlElement| {
                el.get_bounding_client_rect().height() > 0. && el.class_name() != "skip"
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        event.prevent_default();
        let current = targets
            .iter()
            .position(|el| active.as_ref().is_some_and(|a| a.is_same_node(Some(el))));
        let Some(current) = current else {
            let _ = targets[0].focus();
            return;
        };
        let boxes: Vec<_> = targets
            .iter()
            .map(|el| {
                let r = el.get_bounding_client_rect();
                FocusBox {
                    x: r.x(),
                    y: r.y(),
                    width: r.width(),
                    height: r.height(),
                }
            })
            .collect();
        if let Some(next) = nearest_in_direction(current, &boxes, direction) {
            let _ = targets[next].focus();
        }
    });
    vec![hashes, keys]
}
pub fn focus_route(cx: &AppContext, route: &Route, tv: bool) {
    let id = cx.focus_history.borrow().get(&route.href()).cloned();
    let callback = Closure::once_into_js(move || {
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| {
                id.as_deref()
                    .and_then(|id| d.get_element_by_id(id))
                    .or_else(|| {
                        if tv {
                            d.query_selector(".hero .button")
                                .ok()
                                .flatten()
                                .or_else(|| {
                                    d.query_selector("main .button, main button").ok().flatten()
                                })
                        } else {
                            None
                        }
                    })
                    .or_else(|| d.get_element_by_id("page-title"))
            })
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    });
    if let Some(w) = web_sys::window() {
        let _ = w.request_animation_frame(callback.unchecked_ref());
    }
}
