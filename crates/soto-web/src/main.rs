mod api;
#[cfg(target_arch = "wasm32")]
mod browser;
mod components;
#[cfg(target_arch = "wasm32")]
mod hls;
mod pages;
mod state;
use dioxus::prelude::*;
use soto_core::*;
use state::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
fn main() {
    #[cfg(target_arch = "wasm32")]
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        doc.set_title("Soto · Tu biblioteca");
        if let Some(html) = doc.document_element() {
            let _ = html.set_attribute("lang", "es");
        }
        if let Ok(Some(head)) = doc.query_selector("head") {
            for (rel, href) in [
                ("stylesheet", asset!("/assets/style.css").to_string()),
                ("icon", asset!("/assets/favicon.svg").to_string()),
            ] {
                if let Ok(link) = doc.create_element("link") {
                    let _ = link.set_attribute("rel", rel);
                    let _ = link.set_attribute("href", &href);
                    let _ = head.append_child(&link);
                }
            }
        }
    }
    dioxus::launch(App);
}
#[component]
fn App() -> Element {
    let route = use_signal(|| {
        #[cfg(target_arch = "wasm32")]
        {
            Route::parse(
                &web_sys::window()
                    .and_then(|w| w.location().hash().ok())
                    .unwrap_or_default(),
            )
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Route::default()
        }
    });
    let preferences = use_signal(load_preferences);
    let mut cx = use_context_provider(|| AppContext {
        route,
        preferences,
        modal: Signal::new(None),
        session: Signal::new(None),
        revision: Signal::new(0),
        player: Signal::new(None),
        focus_history: Rc::new(RefCell::new(HashMap::new())),
    });
    #[cfg(target_arch = "wasm32")]
    {
        let listener_cx = cx.clone();
        use_hook(move || Rc::new(browser::listeners(listener_cx)));
        let focus_cx = cx.clone();
        use_effect(move || {
            browser::focus_route(&focus_cx, &route.read(), preferences.read().tv);
        });
    }
    let mut boot = use_resource(move || async move {
        let setup: api::Setup = api::request("GET", "/setup", None).await?;
        if setup.configured {
            match api::request::<api::Session>("GET", "/session", None).await {
                Ok(session) => cx.session.set(Some(session)),
                Err(e) if e.status == 401 => {}
                Err(e) => return Err(e),
            }
        }
        Ok::<_, api::ApiError>(setup)
    });
    let modal = cx.modal.read().clone();
    let session = cx.session.read().clone();
    rsx! {
    div{class:if preferences.read().tv{"app tv"}else{"app"},
     if let Some(session)=session {
      pages::Sidebar {}
      pages::Topbar {session}
      main{id:"main-content",class:"workspace",pages::RouteView {}}
     }else{main{class:"auth-shell",
      match &*boot.read(){None=>rsx!{pages::EmptyState{title:"Conectando con Soto…",message:"Comprobando la configuración del servidor."}},Some(Err(error))=>rsx!{pages::EmptyState{title:"Servidor no disponible",message:error.message.clone()}button{class:"button",onclick:move |_|boot.restart(),"Reintentar"}},Some(Ok(setup))=>rsx!{pages::Auth{configured:setup.configured,on_authenticated:move |_|boot.restart()}}}
     }}
     if cx.player.read().is_some(){pages::Player {}}
     if let Some(info)=modal{div{class:"modal-backdrop",role:"presentation",section{class:"functional-dialog",role:"alertdialog","aria-modal":"true","aria-labelledby":"dialog-title",h2{id:"dialog-title","{info.title}"}p{"{info.message}"}button{class:"button",autofocus:true,onclick:move |_|cx.modal.set(None),"Cerrar"}}}}
    }
    }
}
