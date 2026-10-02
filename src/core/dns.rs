//! # Módulo de Consulta e Diagnóstico DNS via DoH (`core/dns.rs`)
//!
//! Permite verificar se os subdomínios do DuckDNS já propagaram nos servidores DNS mundiais
//! utilizando DNS-over-HTTPS (DoH) via Cloudflare e Google, sem bloqueios de firewall UDP porta 53.

use std::time::Duration;
use ureq::Agent;

#[derive(Debug, Clone)]
pub struct DnsCheckResult {
    pub fqdn: String,
    pub resolved_ip: Option<String>,
    pub expected_ip: Option<String>,
    pub is_propagated: bool,
    pub error: Option<String>,
}

/// Consulta a resolução DNS de um subdomínio DuckDNS usando DoH (Cloudflare com fallback Google).
pub fn check_dns_propagation(domain: &str, expected_ip: Option<&str>) -> DnsCheckResult {
    let clean = domain.trim().trim_end_matches('.');
    let fqdn = if clean.ends_with(".duckdns.org") {
        clean.to_string()
    } else {
        format!("{}.duckdns.org", clean)
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build();

    // 1. Tenta Cloudflare DoH
    let resolved = query_cloudflare_doh(&agent, &fqdn)
        // 2. Fallback para Google DoH
        .or_else(|| query_google_doh(&agent, &fqdn));

    match resolved {
        Some(ip) => {
            let is_propagated = match expected_ip {
                Some(exp) => exp.trim() == ip.trim(),
                None => true,
            };
            DnsCheckResult {
                fqdn,
                resolved_ip: Some(ip),
                expected_ip: expected_ip.map(|s| s.to_string()),
                is_propagated,
                error: None,
            }
        }
        None => DnsCheckResult {
            fqdn,
            resolved_ip: None,
            expected_ip: expected_ip.map(|s| s.to_string()),
            is_propagated: false,
            error: Some("Não foi possível resolver o domínio via DoH (DNS).".to_string()),
        },
    }
}

fn query_cloudflare_doh(agent: &Agent, fqdn: &str) -> Option<String> {
    let url = format!("https://cloudflare-dns.com/dns-query?name={}&type=A", fqdn);
    let resp = agent
        .get(&url)
        .set("Accept", "application/dns-json")
        .call()
        .ok()?;

    let json: serde_json::Value = resp.into_json().ok()?;
    extract_ip_from_doh_json(&json)
}

fn query_google_doh(agent: &Agent, fqdn: &str) -> Option<String> {
    let url = format!("https://dns.google/resolve?name={}&type=A", fqdn);
    let resp = agent
        .get(&url)
        .set("Accept", "application/dns-json")
        .call()
        .ok()?;

    let json: serde_json::Value = resp.into_json().ok()?;
    extract_ip_from_doh_json(&json)
}

fn extract_ip_from_doh_json(json: &serde_json::Value) -> Option<String> {
    let answers = json.get("Answer")?.as_array()?;
    for answer in answers {
        // Type 1 is DNS A Record (IPv4)
        if answer.get("type").and_then(|t| t.as_u64()) == Some(1) {
            if let Some(ip) = answer.get("data").and_then(|d| d.as_str()) {
                let clean_ip = ip.trim();
                if clean_ip.parse::<std::net::Ipv4Addr>().is_ok() {
                    return Some(clean_ip.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ip_from_doh_json() {
        let sample = serde_json::json!({
            "Status": 0,
            "Answer": [
                { "name": "duckdns.org.", "type": 1, "data": "16.52.253.242" }
            ]
        });
        assert_eq!(
            extract_ip_from_doh_json(&sample),
            Some("16.52.253.242".to_string())
        );
    }
}
