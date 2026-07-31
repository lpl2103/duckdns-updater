use std::time::Duration;
use ureq::Agent;
use super::config::AppConfig;

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub success: bool,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub _response: String,
}


pub struct DuckDnsService {
    agent: Agent,
}

impl DuckDnsService {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .build();
        Self { agent }
    }

    pub fn get_public_ips(&self) -> (Option<String>, Option<String>) {
        let ipv4 = self
            .agent
            .get("https://api.ipify.org")
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .map(|s| s.trim().to_string());

        let ipv6 = self
            .agent
            .get("https://api6.ipify.org")
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .map(|s| s.trim().to_string());

        (ipv4, ipv6)
    }

    pub fn update(&self, config: &AppConfig) -> Result<UpdateResult, String> {
        let domain = config.domain.trim();
        let token = config.token.trim();

        if domain.is_empty() {
            return Err("O domínio não pode estar vazio.".to_string());
        }
        if token.is_empty() {
            return Err("O token não pode estar vazio.".to_string());
        }

        let (ipv4, ipv6) = self.get_public_ips();

        let mut req = self
            .agent
            .get("https://www.duckdns.org/update")
            .query("domains", domain)
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

        let success = response.to_uppercase().contains("OK");

        Ok(UpdateResult {
            success,
            ipv4,
            ipv6,
            _response: response,
        })

    }
}
