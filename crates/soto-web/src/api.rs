use serde_json::Value;
#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
pub use soto_core::api::*;
pub fn segment(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub async fn request<T: serde::de::DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<T, ApiError> {
    let value = request_value(method, path, body).await?;
    serde_json::from_value(value).map_err(|_| ApiError {
        status: 502,
        message: "Respuesta del servidor no válida".into(),
    })
}
#[cfg(target_arch = "wasm32")]
pub async fn request_value(
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, ApiError> {
    use gloo_net::http::Request;
    let mut request = match method {
        "POST" => Request::post(&format!("/api/v1{path}")),
        "PUT" => Request::put(&format!("/api/v1{path}")),
        "DELETE" => Request::delete(&format!("/api/v1{path}")),
        _ => Request::get(&format!("/api/v1{path}")),
    }
    .header("x-soto-request", "1")
    .credentials(web_sys::RequestCredentials::Omit);
    let response = if let Some(body) = body {
        request = request.header("Content-Type", "application/json");
        request
            .body(body.to_string())
            .map_err(|e| ApiError {
                status: 0,
                message: e.to_string(),
            })?
            .send()
            .await
    } else {
        request.send().await
    }
    .map_err(|_| ApiError {
        status: 0,
        message: "Sin conexión con Soto. Comprueba el servidor y vuelve a intentar.".into(),
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|_| ApiError {
        status,
        message: "No se pudo leer la respuesta".into(),
    })?;
    let value = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).map_err(|_| ApiError {
            status,
            message: "Respuesta del servidor no válida".into(),
        })?
    };
    if !(200..300).contains(&status) {
        return Err(ApiError {
            status,
            message: value["message"]
                .as_str()
                .unwrap_or("La operación no pudo completarse")
                .into(),
        });
    }
    Ok(value)
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn request_value(
    _method: &str,
    _path: &str,
    _body: Option<Value>,
) -> Result<Value, ApiError> {
    Err(ApiError {
        status: 0,
        message: "Conecta con el servidor desde el navegador".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_metadata_is_not_invented() {
        let media: Media = serde_json::from_str(
            r#"{"id":"1","title":"Archivo","kind":"movie","origin":"local","source_count":1}"#,
        )
        .unwrap();
        assert_eq!(media.year, None);
        assert_eq!(media.rating, None);
        assert_eq!(media.description, None);
    }
    #[test]
    fn segments_are_encoded() {
        assert_eq!(segment("a/b &"), "a%2Fb%20%26");
    }
}
