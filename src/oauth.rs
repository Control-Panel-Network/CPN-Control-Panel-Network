use serde::{Deserialize, Serialize};

const DEFAULT_BRIDGE: &str = "https://panel.discord-bot-network.com/api/cloudflare/oauth";

#[derive(Debug, Clone)]
pub struct PendingOAuth {
    pub session_id: String,
    pub claim_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareAuthorization {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub zone_id: String,
    pub zone_name: String,
}

#[derive(Debug, Serialize)]
struct StartRequest<'a> {
    installer_callback: &'a str,
    domain: &'a str,
}

#[derive(Debug, Deserialize)]
struct StartResponse {
    session_id: String,
    claim_secret: String,
    authorization_url: String,
}

#[derive(Debug, Serialize)]
struct ClaimRequest<'a> {
    session_id: &'a str,
    claim_code: &'a str,
    claim_secret: &'a str,
}

#[derive(Debug, Deserialize)]
struct ClaimResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    domain: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareEnvelope<T> {
    success: bool,
    result: T,
    #[serde(default)]
    errors: Vec<CloudflareError>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct Zone {
    id: String,
    name: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())
}

fn bridge_url(path: &str) -> String {
    let base = std::env::var("CPN_OAUTH_BRIDGE_URL").unwrap_or_else(|_| DEFAULT_BRIDGE.into());
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

pub async fn start(
    installer_callback: &str,
    domain: &str,
) -> Result<(PendingOAuth, String), String> {
    let response = client()?
        .post(bridge_url("sessions"))
        .json(&StartRequest {
            installer_callback,
            domain,
        })
        .send()
        .await
        .map_err(|error| format!("No se pudo contactar el puente OAuth: {error}"))?;
    let status = response.status();
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("El puente OAuth devolvió una respuesta inválida: {error}"))?;
    if !status.is_success() {
        return Err(payload
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("El puente OAuth rechazó la solicitud")
            .to_owned());
    }
    let started: StartResponse =
        serde_json::from_value(payload).map_err(|error| error.to_string())?;
    Ok((
        PendingOAuth {
            session_id: started.session_id,
            claim_secret: started.claim_secret,
        },
        started.authorization_url,
    ))
}

fn zone_candidate(domain: &str, zone: &Zone) -> bool {
    domain == zone.name || domain.ends_with(&format!(".{}", zone.name))
}

async fn validate_token(token: &str, domain: &str) -> Result<(String, String), String> {
    let response = client()?
        .get("https://api.cloudflare.com/client/v4/zones")
        .bearer_auth(token)
        .query(&[("per_page", "50")])
        .send()
        .await
        .map_err(|error| format!("No se pudo validar Cloudflare: {error}"))?;
    let envelope: CloudflareEnvelope<Vec<Zone>> = response
        .json()
        .await
        .map_err(|error| format!("Cloudflare devolvió una respuesta inválida: {error}"))?;
    if !envelope.success {
        let detail = envelope
            .errors
            .first()
            .map(|error| error.message.as_str())
            .unwrap_or("credenciales rechazadas");
        return Err(format!("Cloudflare rechazó las credenciales: {detail}"));
    }
    let zone = envelope
        .result
        .into_iter()
        .filter(|zone| zone_candidate(domain, zone))
        .max_by_key(|zone| zone.name.len())
        .ok_or("La autorización no permite acceder a la zona de este dominio")?;
    Ok((zone.id, zone.name))
}

pub async fn claim(
    pending: &PendingOAuth,
    claim_code: &str,
    expected_domain: &str,
) -> Result<CloudflareAuthorization, String> {
    let response = client()?
        .post(bridge_url("claim"))
        .json(&ClaimRequest {
            session_id: &pending.session_id,
            claim_code,
            claim_secret: &pending.claim_secret,
        })
        .send()
        .await
        .map_err(|error| format!("No se pudieron reclamar las credenciales: {error}"))?;
    let status = response.status();
    let payload: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(payload
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("La reclamación OAuth fue rechazada")
            .to_owned());
    }
    let claimed: ClaimResponse =
        serde_json::from_value(payload).map_err(|error| error.to_string())?;
    if claimed.domain != expected_domain {
        return Err("El dominio autorizado no coincide con el dominio del instalador".into());
    }
    let (zone_id, zone_name) = validate_token(&claimed.access_token, expected_domain).await?;
    Ok(CloudflareAuthorization {
        access_token: claimed.access_token,
        refresh_token: claimed.refresh_token,
        expires_in: claimed.expires_in,
        scope: claimed.scope,
        zone_id,
        zone_name,
    })
}
