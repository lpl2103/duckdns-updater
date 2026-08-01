use std::time::Duration;
use std::thread;
use ureq::Agent;
use super::config::AppConfig;

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub success: bool,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub ip_changed: bool,
    pub _response: String,
}

pub struct DuckDnsService {
    agent: Agent,
}

/// Maximum retry attempts on failure.
const MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff (seconds): 5, 15, 45.
const BASE_RETRY_DELAY_SECS: u64 = 5;

impl DuckDnsService {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .build();
        Self { agent }
    }

    /// Fetch the current public IPv4 address.
    pub fn get_public_ipv4(&self) -> Option<String> {
        self.agent
            .get("https://api.ipify.org")
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .map(|s| s.trim().to_string())
    }

    /// Fetch the current public IPv6 address.
    pub fn get_public_ipv6(&self) -> Option<String> {
        self.agent
            .get("https://api6.ipify.org")
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .map(|s| s.trim().to_string())
    }

    /// Perform an update with smart IP change detection, IPv6 toggle, and
    /// multi-domain support. Retries with exponential backoff on failure.
    pub fn update(&self, config: &AppConfig) -> Result<UpdateResult, String> {
        let domains = config.domains_csv();
        let domains = domains.trim();
        let token = config.token.trim();

        if domains.is_empty() {
            return Err("O domínio não pode estar vazio.".to_string());
        }
        if token.is_empty() {
            return Err("O token não pode estar vazio.".to_string());
        }

        // ── Fetch current IPs ──────────────────────────────────────────────
        let ipv4 = self.get_public_ipv4();
        let ipv6 = if config.ipv6_enabled {
            self.get_public_ipv6()
        } else {
            None
        };

        // ── Smart change detection ─────────────────────────────────────────
        let ipv4_changed = ipv4.as_deref() != config.last_ipv4.as_deref();
        let ipv6_changed = if config.ipv6_enabled {
            ipv6.as_deref() != config.last_ipv6.as_deref()
        } else {
            false
        };
        let ip_changed = ipv4_changed || ipv6_changed;

        // Even if IP hasn't changed, still update DuckDNS periodically to
        // keep the record alive. We just flag whether it actually changed.

        // ── Retry loop with exponential backoff ────────────────────────────
        let mut last_err = String::new();
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_RETRY_DELAY_SECS * 3u64.pow(attempt - 1);
                thread::sleep(Duration::from_secs(delay));
            }

            match self.do_update(domains, token, &ipv4, &ipv6) {
                Ok(response) => {
                    let success = response.to_uppercase().contains("OK");
                    return Ok(UpdateResult {
                        success,
                        ipv4,
                        ipv6,
                        ip_changed,
                        _response: response,
                    });
                }
                Err(e) => {
                    last_err = e;
                    if attempt < MAX_RETRIES {
                        continue;
                    }
                }
            }
        }

        Err(format!(
            "Falha após {} tentativas: {}",
            MAX_RETRIES + 1,
            last_err
        ))
    }

    /// Single HTTP request to the DuckDNS update endpoint.
    fn do_update(
        &self,
        domains: &str,
        token: &str,
        ipv4: &Option<String>,
        ipv6: &Option<String>,
    ) -> Result<String, String> {
        let mut req = self
            .agent
            .get("https://www.duckdns.org/update")
            .query("domains", domains)
            .query("token", token)
            .query("verbose", "true");

        if let Some(ref ip4) = ipv4 {
            req = req.query("ip", ip4);
        }

        if let Some(ref ip6) = ipv6 {
            req = req.query("ipv6", ip6);
        }

        let response = req
            .call()
            .map_err(|e| format!("Erro de conexão HTTP: {}", e))?
            .into_string()
            .map_err(|e| format!("Erro ao ler resposta: {}", e))?;

        Ok(response)
    }
}
