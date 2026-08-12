use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct DomainValidation {
    pub valid: bool,
    pub resolvable: bool,
    pub cloudflare: bool,
    pub normalized: Option<String>,
    pub nameservers: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DnsAnswer {
    data: String,
}

#[derive(Debug, Deserialize)]
struct DnsResponse {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "Answer", default)]
    answer: Vec<DnsAnswer>,
}

pub fn normalize_domain(input: &str) -> Result<String, String> {
    let domain = input.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.len() < 4 || domain.len() > 253 || !domain.contains('.') {
        return Err("Escribe un dominio completo, por ejemplo example.com".into());
    }
    if domain.contains("://") || domain.contains('/') || domain.contains('@') {
        return Err("Escribe solo el dominio, sin protocolo, ruta ni correo".into());
    }
    for label in domain.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || value == b'-')
        {
            return Err("El dominio contiene una etiqueta no válida".into());
        }
    }
    Ok(domain)
}

async fn query_nameservers(client: &reqwest::Client, domain: &str) -> Result<Vec<String>, String> {
    let response = client
        .get("https://cloudflare-dns.com/dns-query")
        .query(&[("name", domain), ("type", "NS")])
        .header("Accept", "application/dns-json")
        .send()
        .await
        .map_err(|error| format!("No se pudo consultar DNS: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("El resolvedor DNS respondió {}", response.status()));
    }
    let payload: DnsResponse = response
        .json()
        .await
        .map_err(|error| format!("La respuesta DNS no fue válida: {error}"))?;
    if payload.status != 0 {
        return Ok(Vec::new());
    }
    let mut nameservers = payload
        .answer
        .into_iter()
        .map(|answer| answer.data.trim_end_matches('.').to_ascii_lowercase())
        .collect::<Vec<_>>();
    nameservers.sort();
    nameservers.dedup();
    Ok(nameservers)
}

pub async fn validate_domain(input: &str) -> DomainValidation {
    let normalized = match normalize_domain(input) {
        Ok(value) => value,
        Err(error) => {
            return DomainValidation {
                valid: false,
                resolvable: false,
                cloudflare: false,
                normalized: None,
                nameservers: Vec::new(),
                error: Some(error),
            };
        }
    };
    let client = match reqwest::Client::builder()
        .https_only(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return DomainValidation {
                valid: false,
                resolvable: false,
                cloudflare: false,
                normalized: Some(normalized),
                nameservers: Vec::new(),
                error: Some(error.to_string()),
            };
        }
    };

    let labels = normalized.split('.').collect::<Vec<_>>();
    let mut nameservers = Vec::new();
    for start in 0..labels.len().saturating_sub(1) {
        let candidate = labels[start..].join(".");
        match query_nameservers(&client, &candidate).await {
            Ok(values) if !values.is_empty() => {
                nameservers = values;
                break;
            }
            Ok(_) => {}
            Err(error) => {
                return DomainValidation {
                    valid: false,
                    resolvable: false,
                    cloudflare: false,
                    normalized: Some(normalized),
                    nameservers: Vec::new(),
                    error: Some(error),
                };
            }
        }
    }
    let unique = nameservers.iter().cloned().collect::<HashSet<_>>();
    let cloudflare = !unique.is_empty()
        && unique
            .iter()
            .all(|nameserver| nameserver.ends_with(".cloudflare.com"));
    let resolvable = !nameservers.is_empty();
    DomainValidation {
        valid: resolvable,
        resolvable,
        cloudflare,
        normalized: Some(normalized),
        nameservers,
        error: (!resolvable).then(|| "El dominio no tiene servidores DNS autoritativos".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_domain;

    #[test]
    fn validates_and_normalizes_domains() {
        assert_eq!(normalize_domain(" Example.COM. ").unwrap(), "example.com");
        assert!(normalize_domain("https://example.com").is_err());
        assert!(normalize_domain("-bad.example").is_err());
        assert!(normalize_domain("localhost").is_err());
    }
}
