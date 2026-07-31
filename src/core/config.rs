use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub domain: String,
    pub token: String,
    pub interval_minutes: u32,
    pub last_update: Option<String>,
    pub last_ipv4: Option<String>,
    pub last_ipv6: Option<String>,
    pub update_enabled: bool,
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
        }
    }
}

impl AppConfig {
    pub fn get_config_path() -> PathBuf {
        if let Some(mut dir) = dirs::config_dir() {
            dir.push("duckdns-updater");
            let _ = fs::create_dir_all(&dir);
            dir.push("duckdns_config.json");
            return dir;
        }

        // Fallback: directory of current executable
        if let Ok(mut path) = std::env::current_exe() {
            path.pop();
            path.push("duckdns_config.json");
            return path;
        }

        PathBuf::from("duckdns_config.json")
    }

    pub fn load() -> Self {
        let path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                return config;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_config_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao serializar configuração: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Falha ao salvar arquivo de configuração: {}", e))
    }
}
