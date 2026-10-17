//! Minimal fMP4 HLS adapter using typed browser APIs; no JavaScript player runtime.
use std::{cell::Cell, collections::HashSet, rc::Rc};
use wasm_bindgen::JsValue;
use web_sys::{EventTarget, HtmlVideoElement, MediaSource, SourceBuffer};

async fn event(target: &EventTarget, name: &'static str) -> Result<(), JsValue> {
    event_with_timeout(target, name, 15_000).await
}
async fn event_with_timeout(
    target: &EventTarget,
    name: &'static str,
    millis: u32,
) -> Result<(), JsValue> {
    let (tx, rx) = futures_channel::oneshot::channel();
    let listener = gloo_events::EventListener::once(target, name, move |_| {
        let _ = tx.send(());
    });
    let timeout = gloo_timers::future::TimeoutFuture::new(millis);
    let result = futures_util::future::select(rx, timeout).await;
    drop(listener);
    match result {
        futures_util::future::Either::Left((Ok(()), _)) => Ok(()),
        futures_util::future::Either::Left((Err(_), _)) => {
            Err(JsValue::from_str("Evento multimedia cancelado"))
        }
        futures_util::future::Either::Right(_) => {
            Err(JsValue::from_str("Evento multimedia agotado"))
        }
    }
}
struct ObjectUrl(String);
impl ObjectUrl {
    fn create(media: &MediaSource) -> Result<Self, JsValue> {
        Ok(Self(web_sys::Url::create_object_url_with_source(media)?))
    }
}
impl Drop for ObjectUrl {
    fn drop(&mut self) {
        let _ = web_sys::Url::revoke_object_url(&self.0);
    }
}
async fn binary(url: &str) -> Result<Vec<u8>, JsValue> {
    let response = gloo_net::http::Request::get(url)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .send()
        .await
        .map_err(|_| JsValue::from_str("Fuente no disponible"))?;
    if !response.ok() {
        return Err(JsValue::from_str("Segmento no disponible"));
    }
    response
        .binary()
        .await
        .map_err(|_| JsValue::from_str("Segmento inválido"))
}
async fn append(buffer: &SourceBuffer, data: &[u8]) -> Result<(), JsValue> {
    if buffer.updating() {
        event(buffer.as_ref(), "updateend").await?
    }
    let mut bytes = data.to_vec();
    buffer.append_buffer_with_u8_array(&mut bytes)?;
    event(buffer.as_ref(), "updateend").await
}
pub async fn attach(
    video: HtmlVideoElement,
    url: String,
    cancel: Rc<Cell<bool>>,
    position: f64,
) -> Result<(), JsValue> {
    if !video
        .can_play_type("application/vnd.apple.mpegurl")
        .is_empty()
    {
        video.set_src(&url);
        let _ = video.play();
        return Ok(());
    }
    let media = MediaSource::new()?;
    let object_url = ObjectUrl::create(&media)?;
    video.set_src(&object_url.0);
    event(media.as_ref(), "sourceopen").await?;
    let buffer = media.add_source_buffer("video/mp4; codecs=\"avc1.42E01F,mp4a.40.2\"")?;
    let base = url
        .rsplit_once('/')
        .ok_or_else(|| JsValue::from_str("Playlist inválida"))?
        .0
        .to_string();
    let mut seen = HashSet::new();
    let mut initialized = false;
    let mut resumed = position < 1.;
    let mut started = false;
    while !cancel.get() {
        let response = gloo_net::http::Request::get(&url)
            .credentials(web_sys::RequestCredentials::SameOrigin)
            .send()
            .await
            .map_err(|_| JsValue::from_str("Conversión desconectada"))?;
        if !response.ok() {
            return Err(JsValue::from_str("Conversión no disponible"));
        }
        let playlist = response
            .text()
            .await
            .map_err(|_| JsValue::from_str("Playlist inválida"))?;
        if playlist.len() > 1024 * 1024 {
            return Err(JsValue::from_str("Playlist demasiado grande"));
        }
        if !initialized && playlist.contains("#EXT-X-MAP:") {
            append(&buffer, &binary(&format!("{base}/init.mp4")).await?).await?;
            initialized = true;
        }
        for file in playlist
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        {
            if cancel.get() {
                break;
            }
            if file.contains(['/', '\\']) || file.contains("..") || !file.ends_with(".m4s") {
                return Err(JsValue::from_str("Segmento no permitido"));
            }
            if seen.insert(file.to_string()) {
                loop {
                    let ranges = video.buffered();
                    let end = if ranges.length() > 0 {
                        ranges.end(ranges.length() - 1).unwrap_or(0.)
                    } else {
                        0.
                    };
                    if end < video.current_time() + 90. || cancel.get() {
                        break;
                    }
                    gloo_timers::future::TimeoutFuture::new(300).await;
                }
                append(&buffer, &binary(&format!("{base}/{file}")).await?).await?;
                if !started {
                    let _ = video.play();
                    started = true;
                }
                if !resumed {
                    let ranges = video.buffered();
                    if ranges.length() > 0
                        && ranges.end(ranges.length() - 1).unwrap_or(0.) > position
                    {
                        video.set_current_time(position);
                        resumed = true;
                    }
                }
            }
        }
        if playlist.contains("#EXT-X-ENDLIST") {
            media.end_of_stream()?;
            break;
        }
        gloo_timers::future::TimeoutFuture::new(500).await;
    }
    Ok(())
}
