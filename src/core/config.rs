use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Legacy single-domain field (kept for backward compat on load).
    #[serde(default)]
    pub domain: String,

    pub token: String,
    pub interval_minutes: u32,
    pub last_update: Option<String>,
    pub last_ipv4: Option<String>,
    pub last_ipv6: Option<String>,
    pub update_enabled: bool,

    // ── New fields ──────────────────────────────────────────────────────────
    /// Multiple domains (comma-separated in the UI, stored as Vec).
    #[serde(default)]
    pub domains: Vec<String>,

    /// Register in Windows startup (Registry Run key).
    #[serde(default)]
    pub start_with_windows: bool,

    /// Start the app minimized to the system tray.
    #[serde(default)]
    pub start_minimized: bool,

    /// Enable IPv6 address detection and update.
    #[serde(default = "default_true")]
    pub ipv6_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            token: String::new(),
            interval_minutes: 30,
            last_update: None,
            last_ipv4: None,
            last_ipv6: None,
            update_enabled: true,
            domains: Vec::new(),
            start_with_windows: false,
            start_minimized: false,
            ipv6_enabled: true,
        }
    }
}

impl AppConfig {
    pub fn get_config_dir() -> PathBuf {
        if let Some(mut dir) = dirs::config_dir() {
            dir.push("duckdns-updater");
            let _ = fs::create_dir_all(&dir);
            return dir;
        }

        // Fallback: directory of current executable
        if let Ok(mut path) = std::env::current_exe() {
            path.pop();
            return path;
        }

        PathBuf::from(".")
    }

    pub fn get_config_path() -> PathBuf {
        let mut dir = Self::get_config_dir();
        dir.push("duckdns_config.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                // ── Migration: single domain → multi-domain ────────────
                if config.domains.is_empty() && !config.domain.is_empty() {
                    config.domains = config
                        .domain
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                return config;
            }
        }
        Self::default()
    }

    /// Returns the comma-joined domain string for the DuckDNS API.
    pub fn domains_csv(&self) -> String {
        if self.domains.is_empty() {
            self.domain.clone()
        } else {
            self.domains.join(",")
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_config_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao serializar configuração: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Falha ao salvar arquivo de configuração: {}", e))
    }
}
