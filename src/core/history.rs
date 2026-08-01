use crate::core::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A single entry in the update history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub domains: String,
    pub old_ipv4: Option<String>,
    pub new_ipv4: Option<String>,
    pub old_ipv6: Option<String>,
    pub new_ipv6: Option<String>,
    pub success: bool,
    pub message: String,
}

/// Persistent update history (max 100 entries, FIFO).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateHistory {
    pub entries: Vec<HistoryEntry>,
}

const MAX_ENTRIES: usize = 100;

impl UpdateHistory {
    fn get_path() -> PathBuf {
        let mut dir = AppConfig::get_config_dir();
        dir.push("update_history.json");
        dir
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(history) = serde_json::from_str::<UpdateHistory>(&content) {
                return history;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Falha ao serializar histórico: {}", e))?;
        fs::write(&path, json)
            .map_err(|e| format!("Falha ao salvar histórico: {}", e))
    }

    pub fn add_entry(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
        // Keep only the last MAX_ENTRIES
        if self.entries.len() > MAX_ENTRIES {
            let drain_count = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(0..drain_count);
        }
        let _ = self.save();
    }

    /// Export entries as CSV content string.
    pub fn export_csv(&self) -> String {
        let mut csv = String::from("Data/Hora,Domínios,IPv4 Anterior,IPv4 Novo,IPv6 Anterior,IPv6 Novo,Status,Mensagem\n");
        for e in &self.entries {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                Self::csv_escape(&e.timestamp),
                Self::csv_escape(&e.domains),
                Self::csv_escape(e.old_ipv4.as_deref().unwrap_or("")),
                Self::csv_escape(e.new_ipv4.as_deref().unwrap_or("")),
                Self::csv_escape(e.old_ipv6.as_deref().unwrap_or("")),
                Self::csv_escape(e.new_ipv6.as_deref().unwrap_or("")),
                if e.success { "OK" } else { "FALHA" },
                Self::csv_escape(&e.message),
            ));
        }
        csv
    }

    /// Save CSV export to the config directory.
    pub fn save_csv_export(&self) -> Result<PathBuf, String> {
        let mut path = AppConfig::get_config_dir();
        path.push("duckdns_history_export.csv");
        let csv = self.export_csv();
        fs::write(&path, csv)
            .map_err(|e| format!("Falha ao exportar CSV: {}", e))?;
        Ok(path)
    }

    fn csv_escape(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
}
