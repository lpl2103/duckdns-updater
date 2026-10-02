use serde::{Deserialize, Serialize};
use std::borrow::Cow;
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

    // ── Webhook fields ──────────────────────────────────────────────────────
    /// Discord Webhook URL for update notifications.
    #[serde(default)]
    pub discord_webhook: String,

    /// Telegram Bot Token for update notifications.
    #[serde(default)]
    pub telegram_bot_token: String,

    /// Telegram Chat ID to receive update notifications.
    #[serde(default)]
    pub telegram_chat_id: String,

    /// Notify Windows toasts and webhooks only when IP actually changes.
    #[serde(default)]
    pub notify_on_change_only: bool,
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
            discord_webhook: String::new(),
            telegram_bot_token: String::new(),
            telegram_chat_id: String::new(),
            notify_on_change_only: false,
        }
    }
}

impl AppConfig {
    pub fn get_config_dir() -> PathBuf {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let mut dir = PathBuf::from(appdata);
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
                // ── Decrypt credentials with Windows DPAPI if protected ────
                config.token = crate::core::crypto::decrypt_string(&config.token);
                config.discord_webhook = crate::core::crypto::decrypt_string(&config.discord_webhook);
                config.telegram_bot_token = crate::core::crypto::decrypt_string(&config.telegram_bot_token);
                return config;
            }
        }
        Self::default()
    }

    /// Returns the comma-joined domain string for the DuckDNS API.
    ///
    /// Uses `Cow` to avoid allocating when there is only a single domain
    /// (the common case) and the legacy `domain` field is already a `String`.
    pub fn domains_csv(&self) -> Cow<'_, str> {
        if self.domains.is_empty() {
            // Borrow the legacy field directly — zero allocation.
            Cow::Borrowed(&self.domain)
        } else if self.domains.len() == 1 {
            // Single domain — borrow without joining.
            Cow::Borrowed(&self.domains[0])
        } else {
            // Multiple domains — must allocate to join.
            Cow::Owned(self.domains.join(","))
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_config_path();
        let mut save_copy = self.clone();
        save_copy.token = crate::core::crypto::encrypt_string(&self.token);
        save_copy.discord_webhook = crate::core::crypto::encrypt_string(&self.discord_webhook);
        save_copy.telegram_bot_token = crate::core::crypto::encrypt_string(&self.telegram_bot_token);

        let json = serde_json::to_string_pretty(&save_copy)
            .map_err(|e| format!("Falha ao serializar configuração: {}", e))?;

        // Gravação atômica via arquivo temporário
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("Falha ao gravar arquivo temporário de configuração: {}", e))?;
        fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Falha ao substituir arquivo de configuração: {}", e))
    }
}
