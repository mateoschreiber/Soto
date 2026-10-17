use crate::app::ApiError;
use futures_util::StreamExt;
pub async fn external_client(
    value: &str,
    allow_private: bool,
) -> Result<(reqwest::Client, url::Url), ApiError> {
    let url = validate_url(value, allow_private).await?;
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::bad("URL inválida"))?;
    let addresses = tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(443)))
        .await
        .map_err(|_| ApiError::external("DNS no disponible"))?
        .collect::<Vec<_>>();
    // Check again and pin the addresses to the request; don't resolve a second time in reqwest.
    for address in &addresses {
        if !allow_private && !public_ip(address.ip()) {
            return Err(ApiError::forbidden());
        }
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| ApiError::internal("Cliente HTTP no disponible"))?;
    Ok((client, url))
}
pub async fn json_response(response: reqwest::Response) -> Result<serde_json::Value, ApiError> {
    if !response.status().is_success() {
        return Err(ApiError::external(&format!(
            "El proveedor respondió HTTP {}",
            response.status().as_u16()
        )));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| ApiError::external("Respuesta del proveedor interrumpida"))?;
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(ApiError::external(
                "Respuesta del proveedor demasiado grande",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| ApiError::external("JSON del proveedor inválido"))
}
fn public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !ip.is_broadcast()
                && !ip.is_documentation()
                && ip.octets()[0] != 0
                && !(ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]))
        }
        std::net::IpAddr::V6(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_unique_local()
                && !ip.is_unicast_link_local()
                && !ip.is_multicast()
                && ip.to_ipv4_mapped().is_none_or(|i| public_ip(i.into()))
        }
    }
}
pub async fn validate_url(value: &str, allow_private: bool) -> Result<url::Url, ApiError> {
    let url = url::Url::parse(value).map_err(|_| ApiError::bad("URL inválida"))?;
    if !["http", "https"].contains(&url.scheme())
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ApiError::bad("URL HTTP inválida"));
    }
    if !allow_private && url.scheme() != "https" {
        return Err(ApiError::bad("El proveedor debe utilizar HTTPS"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::bad("Falta el servidor"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ApiError::bad("Puerto inválido"))?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ApiError::external("No se puede resolver el proveedor"))?;
    let mut found = false;
    for address in addresses {
        found = true;
        if !allow_private && !public_ip(address.ip()) {
            return Err(ApiError::forbidden());
        }
    }
    if !found {
        return Err(ApiError::external("El proveedor no tiene dirección"));
    }
    Ok(url)
}
