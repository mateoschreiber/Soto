use crate::{api::*, components::Icon, state::*};
use dioxus::prelude::*;
use serde_json::{Value, json};
use soto_core::{Origin, Page, Route};
#[component]
pub fn EmptyState(title: String, message: String) -> Element {
    rsx! {section{class:"empty functional-empty",role:"status",h2{"{title}"}p{"{message}"}}}
}
#[component]
pub fn Sidebar() -> Element {
    let cx = use_context::<AppContext>();
    let r = cx.route.read().clone();
    rsx! {aside{class:"sidebar",a{class:"brand",href:"#/local/home","SOTO."}nav{"aria-label":"Navegación principal",for (page,icon,label,origin) in [(Page::Home,"home","Inicio",r.origin),(Page::Library,"local","Local",Origin::Local),(Page::Library,"torrent","Torrents",Origin::Torrent),(Page::Collections,"collection","Colecciones",r.origin),(Page::Queue,"clock","En proceso",r.origin),(Page::Server,"server","Servidor",r.origin),(Page::Providers,"plug","Fuentes",r.origin),(Page::Profiles,"info","Perfiles",r.origin)]{a{class:"nav-link",id:"nav-{origin.path()}-{page.path()}",href:Route::new(origin,page).href(),"aria-current":if r.page==page&&r.origin==origin{"page"}else{"false"},Icon{name:icon,size:28}span{"{label}"}}}}}}
}
#[component]
pub fn Topbar(session: Session) -> Element {
    let mut cx = use_context::<AppContext>();
    let mut query = use_signal(String::new);
    rsx! {header{class:"topbar",form{class:"global-search",onsubmit:move|e|{e.prevent_default();let mut r=Route::new(cx.route.read().origin,Page::Search);r.query=query();navigate(r);},Icon{name:"search"}label{class:"sr-only",r#for:"query","Buscar"}input{id:"query",r#type:"search",value:"{query}",placeholder:"Buscar en tu biblioteca",oninput:move|e|query.set(e.value())}}div{class:"functional-actions",span{"{session.server_name} · {session.username}"}button{class:"button secondary",onclick:move |_|{cx.preferences.write().tv^=true;persist(&cx.preferences.read());},"Modo TV"}button{class:"button secondary",onclick:move |_|{spawn(async move{if request_value("DELETE","/session",None).await.is_ok(){cx.session.set(None);cx.player.set(None);}});},"Salir"}}}}
}
#[derive(Clone, PartialEq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
    pub initial: String,
    pub choices: Vec<(String, String)>,
}
fn field(key: &str, label: &str, kind: &str) -> Field {
    Field {
        key: key.into(),
        label: label.into(),
        kind: kind.into(),
        required: true,
        initial: String::new(),
        choices: vec![],
    }
}
fn select(key: &str, label: &str, choices: &[(&str, &str)]) -> Field {
    let mut f = field(key, label, "select");
    f.choices = choices
        .iter()
        .map(|(v, l)| (v.to_string(), l.to_string()))
        .collect();
    f.initial = choices.first().map(|v| v.0.to_string()).unwrap_or_default();
    f
}
#[component]
pub fn ApiForm(
    title: String,
    path: String,
    fields: Vec<Field>,
    #[props(default="POST".into())] method: String,
    #[props(default)] defaults: Value,
    #[props(default)] on_success: EventHandler<Value>,
) -> Element {
    let mut values = use_signal(|| {
        fields
            .iter()
            .map(|f| (f.key.clone(), f.initial.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    });
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut cx = use_context::<AppContext>();
    let submitted_fields = fields.clone();
    let setup_form = path.ends_with("/setup");
    let submit = move |e: FormEvent| {
        e.prevent_default();
        if busy() {
            return;
        }
        let mut body = defaults.as_object().cloned().unwrap_or_default();
        for f in &submitted_fields {
            let text = values.read().get(&f.key).cloned().unwrap_or_default();
            let value = match f.kind.as_str() {
                "number" => match text.parse::<u64>() {
                    Ok(v) => json!(v),
                    Err(_) => {
                        error.set(Some(format!("{}: introduce un entero válido", f.label)));
                        return;
                    }
                },
                "checkbox" => json!(text == "true"),
                "ids" => {
                    let parsed = text
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::parse::<u32>)
                        .collect::<Result<Vec<_>, _>>();
                    match parsed {
                        Ok(ids) => json!(ids),
                        Err(_) => {
                            error.set(Some("Usa IDs numéricos separados por comas".into()));
                            return;
                        }
                    }
                }
                _ => json!(text),
            };
            body.insert(f.key.clone(), value);
        }
        busy.set(true);
        error.set(None);
        let path = path.clone();
        let method = method.clone();
        let submitted_fields = submitted_fields.clone();
        spawn(async move {
            match request_value(&method, &path, Some(Value::Object(body))).await {
                Ok(v) => {
                    cx.revision += 1;
                    on_success.call(v);
                    for f in &submitted_fields {
                        if f.kind == "password" {
                            values.write().insert(f.key.clone(), String::new());
                        }
                    }
                }
                Err(e) => {
                    if e.status == 401 {
                        cx.session.set(None);
                    }
                    error.set(Some(e.message));
                }
            }
            busy.set(false);
        });
    };
    rsx! {
     form{class:"functional-form",onsubmit:submit,
      h3{"{title}"}
      for f in fields {
       label{class:"functional-field",span{"{f.label}"}
        if f.kind=="select"{
         select{name:"{f.key}",value:values.read().get(&f.key).cloned().unwrap_or_default(),onchange:{let key=f.key.clone();move|e|{values.write().insert(key.clone(),e.value());}},for (v,l) in f.choices{option{value:v,"{l}"}}}
        }else if f.kind=="checkbox"{
         input{r#type:"checkbox",checked:values.read().get(&f.key).is_some_and(|s|s=="true"),onchange:{let key=f.key.clone();move|e|{values.write().insert(key.clone(),e.value());}}}
        }else{
         input{r#type:if f.kind=="ids"{"text"}else{f.kind.as_str()},name:"{f.key}",autocomplete:if f.key=="username"{"username"}else if f.key=="password"&&setup_form{"new-password"}else if f.key=="password"{"current-password"}else{"off"},required:f.required,value:values.read().get(&f.key).cloned().unwrap_or_default(),minlength:if f.key=="password"{"12"}else{"0"},maxlength:if f.kind=="password"{"1024"}else{"8192"},oninput:{let key=f.key.clone();move|e|{values.write().insert(key.clone(),e.value());}}}
        }
       }
      }
      if let Some(message)=error(){p{class:"form-error",role:"alert","{message}"}}
      button{class:"button",r#type:"submit",disabled:busy(),if busy(){"Guardando…"}else{"Confirmar"}}
     }
    }
}
#[component]
pub fn Auth(configured: bool, on_authenticated: EventHandler) -> Element {
    let fields = if configured {
        vec![
            field("username", "Usuario", "text"),
            field("password", "Contraseña", "password"),
        ]
    } else {
        vec![
            field(
                "setup_token",
                "Token de instalación (registro del servidor)",
                "password",
            ),
            field("server_name", "Nombre del servidor", "text"),
            field("username", "Administrador", "text"),
            field("password", "Contraseña (mínimo 12 caracteres)", "password"),
        ]
    };
    rsx! {div{class:"auth-card",span{class:"brand","SOTO."}h1{if configured{"Tu biblioteca te espera"}else{"Configura tu servidor"}}p{if configured{"Inicia sesión para acceder a tu biblioteca."}else{"Crea la cuenta del administrador. Puedes añadir bibliotecas y servicios después."}}ApiForm{title:if configured{"Iniciar sesión"}else{"Crear administrador"},path:if configured{"/session"}else{"/setup"},fields,on_success:move |_|on_authenticated.call(())}}}
}
#[component]
pub fn RouteView() -> Element {
    let cx = use_context::<AppContext>();
    let route = cx.route.read().clone();
    rsx! {a{class:"skip",href:"#page-title","Saltar al contenido"}if route.page!=Page::Home{div{class:"functional-heading",span{class:"eyebrow","TU BIBLIOTECA, EN TU HOMELAB"}h1{id:"page-title",tabindex:"-1","{page_title(route.page,route.origin)}"}}}match route.page {Page::Home|Page::Library|Page::Search|Page::Collections=>rsx!{Catalog{key:"{route.href()}",route:route.clone()}if route.page==Page::Home{ContinueWatching{}}if route.page==Page::Library&&route.origin==Origin::Torrent{Torrents{}}},Page::Detail|Page::Sources=>rsx!{MediaDetail{key:"{route.href()}",id:route.id.unwrap_or_default()}},Page::Server=>rsx!{Admin{}},Page::Providers=>rsx!{Providers{}},Page::Queue=>rsx!{Records{path:"/jobs",kind:"jobs"}},Page::Profiles=>rsx!{Records{path:"/profiles",kind:"profiles"}ApiForm{title:"Crear perfil",path:"/profiles",fields:vec![field("name","Nombre","text")]}},Page::Devices=>rsx!{EmptyState{title:"Modo TV",message:"Activa Modo TV en la barra superior. Usa las flechas para navegar, Intro para elegir y Escape para volver."}},_=>rsx!{EmptyState{title:"Página no encontrada",message:"Usa la navegación para volver a tu biblioteca."}}}}
}
fn page_title(page: Page, origin: Origin) -> &'static str {
    match page {
        Page::Home => "Tu biblioteca",
        Page::Library if origin == Origin::Torrent => "Torrents",
        Page::Library => "Biblioteca local",
        Page::Search => "Buscar",
        Page::Collections => "Colecciones",
        Page::Detail | Page::Sources => "Detalle y fuentes",
        Page::Server => "Servidor",
        Page::Providers => "Fuentes y Debrid",
        Page::Queue => "En proceso",
        Page::Profiles => "Perfiles",
        _ => "Soto",
    }
}
#[component]
fn Catalog(route: Route) -> Element {
    let cx = use_context::<AppContext>();
    let mut kind = use_signal(|| "all".to_string());
    let request_route = route.clone();
    let mut data = use_resource(move || {
        let r = request_route.clone();
        let k = kind();
        let _ = (cx.revision)();
        async move {
            let path = if r.page == Page::Collections {
                "/collections".into()
            } else {
                format!(
                    "/catalog?origin={}&q={}&kind={}",
                    r.origin.path(),
                    segment(&r.query),
                    k
                )
            };
            request::<Vec<Media>>("GET", &path, None).await
        }
    });
    rsx! {
      match &*data.read() {
        None => rsx! { EmptyState { title:"Cargando biblioteca…", message:"Consultando el servidor." } },
        Some(Err(e)) => rsx! { EmptyState { title:"No se pudo cargar", message:e.message.clone() } },
        Some(Ok(items)) if items.is_empty() => rsx! {
          if route.page==Page::Home { h1 { id:"page-title", tabindex:"-1", class:"empty-page-title", "Tu biblioteca" } }
          EmptyState {
            title:if route.page==Page::Collections {"Tu colección está vacía"} else if route.page==Page::Search {"No hay resultados"} else if route.origin==Origin::Torrent {"Sin títulos disponibles"} else {"Tu biblioteca está vacía"},
            message:if route.page==Page::Collections {"Guarda títulos desde su ficha para tenerlos a mano."} else if route.page==Page::Search {"Prueba con otro título o revisa el origen seleccionado."} else if route.origin==Origin::Torrent {"Instala un proveedor o importa un magnet desde la sección Torrents."} else {"Conecta una carpeta desde Servidor y escanéala. Tus archivos aparecerán aquí."}
          }
          if route.page!=Page::Collections {
            div { class:"catalog-empty-actions",
              if route.origin==Origin::Torrent {a{class:"button",href:"#/local/providers","Configurar proveedores"}} else {a{class:"button",href:"#/local/server","Administrar bibliotecas"}}
            }
          }
        },
        Some(Ok(items)) => rsx! {
          if route.page==Page::Home { Feature { item:items.iter().find(|item|item.poster.is_some()).unwrap_or(&items[0]).clone() } }
          if route.page==Page::Home {
            nav { class:"categories", "aria-label":"Filtrar catálogo",
              for (value,icon,label) in [("all","home","Todo"),("movie","film","Películas"),("series","tv","Series")] {
                button { class:"category", "aria-pressed":kind()==value, onclick:move |_|kind.set(value.to_string()), Icon{name:icon,size:38}, span{"{label}"} }
              }
            }
          } else {
            div { class:"functional-actions catalog-toolbar",
              label { class:"catalog-filter", "Mostrar ", select { value:"{kind}", onchange:move|e|kind.set(e.value()), option{value:"all","Todo"} option{value:"movie","Películas"} option{value:"series","Series"} } }
              button { class:"icon-button", "aria-label":"Actualizar catálogo", onclick:move |_|data.restart(), Icon{name:"arrow",size:24} }
            }
          }
          section { class:"shelves",
            div { class:"shelf-heading", h2{if route.page==Page::Search{"Resultados de búsqueda"}else if route.page==Page::Collections{"Guardados"}else if route.origin==Origin::Torrent{"Descubrimiento"}else{"Añadidas recientemente"}}, span { class:"catalog-count", "{items.len()} títulos" } }
            div { class:"poster-grid", for item in items { MediaCard { item:item.clone() } } }
          }
        }
      }
    }
}

#[component]
fn Feature(item: Media) -> Element {
    let origin = if item.origin == "torrent" {
        Origin::Torrent
    } else {
        Origin::Local
    };
    let route = Route::media(origin, Page::Detail, &item.id).href();
    rsx! {
      section { class:"hero", "aria-label":"Título destacado",
        div { class:"hero-art",
          if item.poster.is_some() { img { src:format!("/api/v1/catalog/{}/poster",segment(&item.id)), alt:"", fetchpriority:"high" } }
          else { div { class:"hero-art-placeholder", "aria-hidden":"true" } }
        }
        div { class:"hero-copy",
          div { class:"hero-tags", span { class:"pill green", if origin==Origin::Torrent {"Torrents"} else {"En tu biblioteca"} } }
          h1 { id:"page-title", tabindex:"-1", "{item.title}" }
          p { class:"hero-meta", "{metadata(&item)}" }
          if let Some(description)=item.description.as_deref().filter(|s|!s.is_empty()) { p { class:"hero-description", "{description}" } }
          div { class:"actions", a { class:"button", href:route.clone(), Icon{name:"play",size:20}, "Ver título" } }
        }
        a { class:"hero-play", href:route, "aria-label":"Abrir ficha de {item.title}", Icon{name:"play",size:34} }
      }
    }
}
#[component]
fn MediaCard(item: Media) -> Element {
    let origin = if item.origin == "torrent" {
        Origin::Torrent
    } else {
        Origin::Local
    };
    rsx! {
      a { class:"poster", id:"media-{item.id}", href:Route::media(origin,Page::Detail,&item.id).href(),
        div { class:"art functional-art",
          if item.poster.is_some() { img { src:format!("/api/v1/catalog/{}/poster",segment(&item.id)), alt:"", loading:"lazy" } }
          else { Icon{name:"film",size:48} }
          span { class:"poster-origin", if origin==Origin::Torrent {"TORRENTS"} else {"LOCAL"} }
          if item.source_count>0 { span { class:"poster-source-count", "{item.source_count} fuentes" } }
        }
        div { class:"poster-footer",
          div { class:"poster-title", strong {"{item.title}"}, small {
            if let Some(year)=item.year {"{year} · "}
            "{item.kind} · {item.source_count} fuente(s)"
          } }
          Icon { name:"arrow", size:18 }
        }
      }
    }
}
#[component]
fn MediaDetail(id: String) -> Element {
    let mut cx = use_context::<AppContext>();
    let mut mode = use_signal(|| "auto".to_string());
    let mut audio = use_signal(String::new);
    let mut engine = use_signal(|| "debrid".to_string());
    let mut busy = use_signal(|| false);
    let request_id = id.clone();
    let mut detail = use_resource(move || {
        let id = request_id.clone();
        let _ = (cx.revision)();
        async move { request::<Detail>("GET", &format!("/catalog/{}", segment(&id)), None).await }
    });
    rsx! {match &*detail.read(){None=>rsx!{EmptyState{title:"Cargando detalle…",message:"Consultando las fuentes disponibles."}},Some(Err(e))=>rsx!{EmptyState{title:"Detalle no disponible",message:e.message.clone()}button{class:"button",onclick:move |_|detail.restart(),"Reintentar"}},Some(Ok(d))=>rsx!{section{class:"functional-panel",h2{"{d.media.title}"}p{"{metadata(&d.media)}"}if let Some(description)=&d.media.description{p{"{description}"}}else{p{"Sin sinopsis disponible."}}div{class:"functional-actions",Action{label:"Guardar en colección",path:"/collections",body:json!({"media_id":id})}Action{label:"Quitar de colección",path:format!("/collections/{}",segment(&id)),method:"DELETE"}Action{label:"Buscar fuentes instaladas",path:format!("/catalog/{}/resolve",segment(&id))}}}
    if !d.episodes.is_empty(){section{class:"functional-panel",h3{"Episodios"}for episode in &d.episodes{a{class:"button secondary",href:Route::media(Origin::Torrent,Page::Detail,&episode.id).href(),"{episode.title}"}}}}
    label{class:"functional-field",span{"Reproducción"}select{value:"{mode}",onchange:move|e|mode.set(e.value()),option{value:"auto","Automática"}option{value:"direct","Directa"}option{value:"convert","Convertir"}}}
    if d.sources.is_empty(){EmptyState{title:"Sin fuentes",message:"Escanea tu biblioteca o busca con los proveedores que hayas instalado."}}
    label{class:"functional-field",span{"Preparar torrents con"}select{value:"{engine}",onchange:move|e|engine.set(e.value()),option{value:"debrid","Real-Debrid"}option{value:"p2p","P2P directo"}}}
    for source in &d.sources{div{class:"functional-row",div{strong{"{source.label}"}p{"{source.kind} · {source.status}"}if let Some(quality)=&source.quality{span{"{quality} "}}if let Some(size)=source.size{span{"{size} bytes "}}if let Some(codec)=&source.video_codec{span{"{codec} "}}if let Some(codec)=&source.audio_codec{span{"{codec}"}}}
    if source.kind=="magnet"{Action{label:"Preparar fuente",path:format!("/sources/{}/prepare",segment(&source.id)),body:json!({"engine":engine()})}}
    if !source.audio_streams.is_empty(){label{class:"functional-field",span{"Audio"}select{value:"{audio}",onchange:move|e|audio.set(e.value()),option{value:"","Predeterminado"}for track in &source.audio_streams{option{value:track.id.clone(),"{track.label}"}}}}}
    button{class:"button",disabled:source.status!="available"||busy(),onclick:{let source_id=source.id.clone();let media_id=id.clone();move |_|{busy.set(true);let source_id=source_id.clone();let media_id=media_id.clone();spawn(async move{match request::<Playback>("POST","/playback",Some(json!({"source_id":source_id,"mode":mode(),"audio_stream":audio().parse::<u32>().ok()}))).await{Ok(session)=>cx.player.set(Some((session,media_id))),Err(e)=>cx.modal.set(Some(DialogInfo{title:"No se pudo iniciar el vídeo".into(),message:e.message}))}busy.set(false);});}},"Reproducir"}}
    }
    }}}
}
fn metadata(media: &Media) -> String {
    let mut result = vec![media.kind.clone(), media.origin.clone()];
    if let Some(year) = media.year {
        result.push(year.to_string());
    }
    if let Some(rating) = &media.rating {
        result.push(rating.clone());
    }
    if let Some(seconds) = media.duration_seconds {
        result.push(format!("{} min", seconds / 60));
    }
    result.join(" · ")
}
#[component]
fn Action(
    label: String,
    path: String,
    #[props(default="POST".into())] method: String,
    #[props(default)] body: Value,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut cx = use_context::<AppContext>();
    rsx! {div{class:"functional-action",button{class:"button secondary",disabled:busy(),onclick:move |_|{busy.set(true);error.set(None);let path=path.clone();let method=method.clone();let body=body.clone();spawn(async move{match request_value(&method,&path,if body.is_null(){None}else{Some(body)}).await{Ok(_)=>cx.revision+=1,Err(e)=>{if e.status==401{cx.session.set(None);}error.set(Some(e.message));}}busy.set(false);});},if busy(){"Procesando…"}else{"{label}"}}if let Some(message)=error(){p{class:"form-error",role:"alert","{message}"}}}}
}
#[component]
fn Records(path: String, kind: String) -> Element {
    let cx = use_context::<AppContext>();
    let request_path = path.clone();
    let mut data = use_resource(move || {
        let path = request_path.clone();
        let _ = (cx.revision)();
        async move { request::<Vec<Value>>("GET", &path, None).await }
    });
    rsx! {div{class:"functional-actions",button{class:"button secondary",onclick:move |_|data.restart(),"Actualizar estado"}}
    match &*data.read(){None=>rsx!{p{role:"status","Cargando…"}},Some(Err(e))=>rsx!{p{class:"form-error",role:"alert","{e.message}"}},Some(Ok(rows)) if rows.is_empty()=>rsx!{EmptyState{title:"Sin registros",message:"Aquí aparecerán los datos guardados en tu servidor."}},Some(Ok(rows))=>rsx!{for row in rows {Record{row:row.clone(),kind:kind.clone(),path:path.clone()}}}}
    }
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().map(str::to_string).unwrap_or_default()
}
#[component]
fn Record(row: Value, kind: String, path: String) -> Element {
    let id = text(&row, "id");
    let item_path = format!("{}/{}", path, segment(&id));
    let name = if row["name"].is_string() {
        text(&row, "name")
    } else {
        format!("{} {}", text(&row, "kind"), id)
    };
    let status = text(&row, "status");
    let active = matches!(
        status.as_str(),
        "queued" | "running" | "downloading" | "paused" | "pending"
    );
    rsx! {section{class:"functional-row",div{h3{"{name}"}p{"{status}"}if let Some(progress)=row["progress"].as_f64(){p{"Progreso: {progress:.1}"}}if let Some(error)=row["error"].as_str(){p{class:"form-error","{error}"}}if kind=="libraries"{p{{text(&row,"path")}}if !row["last_scan"].is_null(){p{{format!("Último escaneo: {}",row["last_scan"])}}}}if kind=="providers"{p{{text(&row,"url")}}}}
     div{class:"functional-actions",
     match kind.as_str(){"libraries"=>rsx!{Action{label:"Escanear",path:format!("{item_path}/scan")}Action{label:"Quitar biblioteca",path:item_path.clone(),method:"DELETE"}},"providers"=>rsx!{Action{label:"Actualizar",path:format!("{item_path}/refresh")}Action{label:"Eliminar proveedor",path:item_path.clone(),method:"DELETE"}},"profiles"=>rsx!{Action{label:"Eliminar perfil",path:item_path.clone(),method:"DELETE"}},"jobs" if active=>rsx!{Action{label:"Cancelar trabajo",path:format!("{item_path}/cancel")}},"torrents"=>rsx!{Action{label:"Pausar",path:format!("{item_path}/pause")}Action{label:"Reanudar",path:format!("{item_path}/resume")}Action{label:"Cancelar torrent",path:format!("{item_path}/cancel")}},_=>rsx!{}}
     }
     if let Some(files)=row["files"].as_array(){div{class:"functional-files",for f in files{p{{format!("ID {} · {} · {} bytes · seleccionado: {}",f["id"],text(f,"name"),f["size"],f["selected"])}}}ApiForm{title:"Seleccionar archivos",path:format!("{item_path}/select"),fields:vec![field("file_ids","IDs de archivos separados por comas","ids")]}}}
     }
    }
}
#[component]
fn Providers() -> Element {
    rsx! {p{"Instala solamente manifiestos JSON de proveedores de confianza. No se ejecuta código de extensiones."}ApiForm{title:"Instalar proveedor",path:"/providers",fields:vec![field("manifest_url","URL del manifiesto","url")]}Records{path:"/providers",kind:"providers"}Debrid{}}
}
#[component]
fn Debrid() -> Element {
    let cx = use_context::<AppContext>();
    let account = use_resource(move || {
        let _ = (cx.revision)();
        async move { request::<Value>("GET", "/debrid", None).await }
    });
    rsx! {section{class:"functional-panel",h2{"Real-Debrid"}match &*account.read(){None=>rsx!{p{"Consultando cuenta…"}},Some(Err(e))=>rsx!{p{role:"alert","{e.message}"}},Some(Ok(a))=>rsx!{p{if a["connected"].as_bool()==Some(true){{format!("Conectado · {} · {}",text(a,"username"),text(a,"status"))}}else{"Sin cuenta conectada"}}if let Some(expiration)=a["premium_until"].as_str(){p{"Premium hasta {expiration}"}}if let Some(error)=a["error"].as_str(){p{role:"alert","{error}"}}}}
    ApiForm{title:"Conectar cuenta",path:"/debrid",fields:vec![field("token","Token de Real-Debrid","password")]}div{class:"functional-actions",Action{label:"Desconectar",path:"/debrid",method:"DELETE"}Action{label:"Sincronizar",path:"/debrid/sync"}}ApiForm{title:"Enviar magnet a Debrid",path:"/debrid/magnet",fields:vec![field("magnet","Magnet","text")]}Records{path:"/debrid/torrents",kind:"debrid"}}}
}
#[component]
fn Torrents() -> Element {
    rsx! {section{class:"functional-panel",h2{"Importar torrent"}p{"El motor P2P requiere habilitar torrents en los ajustes. Real-Debrid requiere una cuenta conectada."}ApiForm{title:"Importar magnet",path:"/torrents",fields:vec![select("engine","Motor",&[("p2p","P2P local"),("debrid","Real-Debrid")]),field("magnet","Magnet","text")]}TorrentFile{}Records{path:"/torrents",kind:"torrents"}}}
}
#[component]
fn TorrentFile() -> Element {
    let mut engine = use_signal(|| "p2p".to_string());
    let mut encoded = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let mut reading = use_signal(|| false);
    rsx! {div{class:"functional-form",h3{"Importar archivo .torrent"}label{class:"functional-field",span{"Motor"}select{value:"{engine}",onchange:move|e|engine.set(e.value()),option{value:"p2p","P2P local"}option{value:"debrid","Real-Debrid"}}}label{class:"functional-field",span{"Archivo .torrent"}input{r#type:"file",accept:".torrent",onchange:move|event|{encoded.set(None);error.set(None);if let Some(file)=event.files().into_iter().next(){reading.set(true);spawn(async move{use base64::Engine;match file.read_bytes().await{Ok(bytes) if bytes.len()<=8*1024*1024=>encoded.set(Some(base64::engine::general_purpose::STANDARD.encode(bytes))),Ok(_)=>error.set(Some("El archivo supera el límite de 8 MiB".into())),Err(_)=>error.set(Some("No se pudo leer el archivo".into()))}reading.set(false);});}}}}if reading(){p{role:"status","Leyendo archivo…"}}if let Some(message)=error(){p{role:"alert","{message}"}}if let Some(torrent)=encoded(){Action{label:"Importar archivo",path:"/torrents",body:json!({"engine":engine(),"torrent_base64":torrent})}}}}
}
#[component]
fn Admin() -> Element {
    let cx = use_context::<AppContext>();
    if !cx.session.read().as_ref().is_some_and(|s| s.admin) {
        return rsx! {EmptyState{title:"Acceso de administrador",message:"La configuración está reservada al administrador del servidor."}};
    }
    rsx! {Settings{}Libraries{}}
}
#[component]
fn Settings() -> Element {
    let cx = use_context::<AppContext>();
    let settings = use_resource(move || {
        let _ = (cx.revision)();
        async move { request::<Value>("GET", "/settings", None).await }
    });
    rsx! {section{class:"functional-panel",h2{"Ajustes"}match &*settings.read(){None=>rsx!{p{"Cargando ajustes…"}},Some(Err(e))=>rsx!{p{role:"alert","{e.message}"}},Some(Ok(value))=>{let fields=[("server_name","Nombre del servidor","text"),("torrent_enabled","Habilitar motor torrent","checkbox"),("torrent_quota_gb","Cuota torrent (GB)","number"),("cache_quota_gb","Cuota caché (GB)","number"),("download_kib","Descarga (KiB/s)","number"),("upload_kib","Subida (KiB/s)","number")].into_iter().map(|(key,label,kind)|{let mut f=field(key,label,kind);f.initial=value[key].as_str().map(str::to_string).unwrap_or_else(||value[key].to_string());f}).collect::<Vec<_>>();rsx!{ApiForm{key:"settings-{cx.revision}",title:"Guardar ajustes",path:"/settings",method:"PUT",fields}}}}
    }}
}
#[component]
fn Libraries() -> Element {
    let roots =
        use_resource(move || async move { request::<Vec<Value>>("GET", "/roots", None).await });
    let mut root = use_signal(String::new);
    let mut path = use_signal(String::new);
    let dirs = use_resource(move || {
        let root = root();
        let path = path();
        async move {
            if root.is_empty() {
                return Ok(vec![]);
            }
            request::<Vec<Value>>(
                "GET",
                &format!(
                    "/libraries/browse?root_id={}&path={}",
                    segment(&root),
                    segment(&path)
                ),
                None,
            )
            .await
        }
    });
    rsx! {section{class:"functional-panel",h2{"Bibliotecas"}p{"Elige una raíz autorizada y una carpeta. Los escaneos reconocen archivos reales del servidor."}
    match &*roots.read(){None=>rsx!{p{"Cargando raíces…"}},Some(Err(e))=>rsx!{p{role:"alert","{e.message}"}},Some(Ok(roots)) if roots.is_empty()=>rsx!{EmptyState{title:"Sin raíces autorizadas",message:"Configura las raíces de medios en el servidor antes de crear una biblioteca."}},Some(Ok(roots))=>rsx!{label{class:"functional-field",span{"Raíz autorizada"}select{value:"{root}",onchange:move|e|{root.set(e.value());path.set(String::new());},option{value:"","Elige una raíz"}for r in roots{option{value:text(r,"id"),{text(r,"name")}}}}}}}
    if !root().is_empty(){p{"Carpeta: /{path}"}button{class:"button secondary",onclick:move |_|{let current=path();path.set(current.rsplit_once('/').map(|(p,_)|p.to_string()).unwrap_or_default());},"Subir carpeta"}match &*dirs.read(){None=>rsx!{p{"Cargando carpetas…"}},Some(Err(e))=>rsx!{p{role:"alert","{e.message}"}},Some(Ok(dirs))=>rsx!{div{class:"functional-actions",for dir in dirs{button{class:"button secondary",onclick:{let next=text(dir,"path");move |_|path.set(next.clone())},{text(dir,"name")}}}}}}
    ApiForm{title:"Añadir esta carpeta",path:"/libraries",defaults:json!({"root_id":root(),"path":path()}),fields:vec![field("name","Nombre de biblioteca","text"),select("kind","Tipo",&[("mixed","Mixta"),("movie","Películas"),("series","Series")])]}}
    Records{path:"/libraries",kind:"libraries"}
    }}
}
#[component]
pub fn Player() -> Element {
    let mut cx = use_context::<AppContext>();
    let Some((session, media_id)) = cx.player.read().clone() else {
        return rsx! {};
    };
    let mut last_saved = use_signal(|| 0_f64);
    let mut error = use_signal(|| false);
    let id = session.id.clone();
    let close_media = media_id.clone();
    let update_media = media_id.clone();
    let pause_media = media_id.clone();
    let start = session.position;
    let is_hls = session.mode == "convert";
    let hls_url = session.url.clone();
    #[cfg(target_arch = "wasm32")]
    let cancel = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    #[cfg(target_arch = "wasm32")]
    {
        let cancel = cancel.clone();
        use_drop(move || cancel.set(true));
    }
    rsx! {div{class:"player-overlay",role:"dialog","aria-modal":"true","aria-label":"Reproductor",div{class:"functional-actions",button{class:"button",autofocus:true,onclick:move |_|{save_progress(&close_media);let path=format!("/playback/{}",segment(&id));spawn(async move{let _=request_value("DELETE",&path,None).await;});cx.player.set(None);cx.revision+=1;},"Cerrar reproductor"}span{"Modo: {session.mode}"}}
    if session.url.starts_with("/api/")&&!session.url.starts_with("//"){
    video{id:"soto-video",src:if is_hls{None}else{Some(session.url)},onmounted:move |_|{
        #[cfg(target_arch="wasm32")]
        if is_hls {let url=hls_url.clone();let cancel=cancel.clone();spawn(async move{if let Some(video)=video_element()&&crate::hls::attach(video,url,cancel,start).await.is_err(){error.set(true);}});}
        #[cfg(not(target_arch="wasm32"))]let _=(&hls_url,is_hls);
    },"data-timeline-offset":session.timeline_offset,"data-total-duration":session.duration.map(|v|v.to_string()).unwrap_or_default(),controls:true,autoplay:true,preload:"metadata",onloadedmetadata:move |_|{#[cfg(target_arch="wasm32")]if let Some(video)=video_element(){video.set_current_time(start.max(0.));} #[cfg(not(target_arch="wasm32"))]let _=start;},ontimeupdate:move |_|{#[cfg(target_arch="wasm32")]if let Some(video)=video_element(){let time=video.current_time();if (time-last_saved()).abs()>=10.{last_saved.set(time);save_progress(&update_media);}}#[cfg(not(target_arch="wasm32"))]let _=(&update_media,&mut last_saved);},onpause:move |_|save_progress(&pause_media),onended:move |_|save_progress(&media_id),onerror:move |_|error.set(true),for track in session.subtitles{track{kind:"subtitles",label:track.label,src:track.url}}}
    }else{p{role:"alert","El servidor devolvió una dirección de reproducción no válida."}}
    if error(){p{role:"alert","El navegador no pudo reproducir esta fuente. Cierra el reproductor y prueba el modo Convertir u otra fuente."}}
    if !session.audio_streams.is_empty(){p{"Pistas de audio disponibles:"}for track in session.audio_streams{span{"{track.label} "}}p{"La selección de pistas depende de los controles nativos del navegador."}}
    }}
}
#[cfg(target_arch = "wasm32")]
fn video_element() -> Option<web_sys::HtmlVideoElement> {
    use wasm_bindgen::JsCast;
    web_sys::window()?
        .document()?
        .get_element_by_id("soto-video")?
        .dyn_into()
        .ok()
}
fn save_progress(media_id: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(video) = video_element() {
        let offset = video
            .get_attribute("data-timeline-offset")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.);
        let position = offset + video.current_time();
        let duration = video.duration();
        let duration = if duration.is_finite() && duration > 0. {
            offset + duration
        } else {
            video
                .get_attribute("data-total-duration")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.)
        };
        if position.is_finite() && duration.is_finite() && duration > 0. {
            let path = format!("/progress/{}", segment(media_id));
            spawn(async move {
                let _ = request_value(
                    "PUT",
                    &path,
                    Some(json!({"position":position,"duration":duration})),
                )
                .await;
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = media_id;
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Element {
        use_context_provider(|| AppContext {
            route: Signal::new(Route::default()),
            preferences: Signal::new(Default::default()),
            modal: Signal::new(None),
            session: Signal::new(None),
            revision: Signal::new(0),
            player: Signal::new(None),
            focus_history: Default::default(),
        });
        rsx! {Auth{configured:false,on_authenticated:move |_|{}}}
    }
    fn empty() -> Element {
        rsx! {EmptyState{title:"Todavía no hay contenido",message:"Añade una biblioteca"}}
    }
    #[test]
    fn clean_setup_has_real_credentials_form() {
        let mut dom = VirtualDom::new(setup);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(html.contains("setup_token"));
        assert!(html.contains("password"));
        assert!(!html.contains("Dune"));
        assert!(!html.contains("Vista previa"));
    }
    #[test]
    fn empty_state_is_explicit() {
        let mut dom = VirtualDom::new(empty);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(html.contains("Todavía no hay contenido"));
        assert!(html.contains("role=\"status\""));
    }
    #[test]
    fn metadata_omits_missing_values() {
        let media: Media = serde_json::from_value(
            json!({"id":"1","title":"Archivo","kind":"movie","origin":"local","source_count":1}),
        )
        .unwrap();
        assert_eq!(metadata(&media), "movie · local");
    }
}

#[component]
fn ContinueWatching() -> Element {
    let cx = use_context::<AppContext>();
    let mut progress = use_resource(move || {
        let _ = (cx.revision)();
        async move { request::<Vec<Value>>("GET", "/progress", None).await }
    });
    rsx! {match &*progress.read(){None=>rsx!{p{role:"status","Cargando progreso…"}},Some(Err(e))=>rsx!{p{role:"alert","{e.message}"}button{class:"button secondary",onclick:move |_|progress.restart(),"Reintentar"}},Some(Ok(rows)) if rows.is_empty()=>rsx!{},Some(Ok(rows))=>rsx!{section{class:"shelves",div{class:"shelf-heading",h2{"Continuar viendo"}}div{class:"continue-list",for row in rows{a{class:"button secondary",href:format!("#/local/detail/{}",segment(&text(row,"media_id"))),{format!("Continuar · {} · {:.0} min",text(row,"media_id"),row["position"].as_f64().unwrap_or(0.) / 60.)}}}}}}}}
}
