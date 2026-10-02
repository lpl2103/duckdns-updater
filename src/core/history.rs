use crate::core::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Write as FmtWrite;
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

        // Gravação atômica via arquivo temporário
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("Falha ao gravar arquivo temporário de histórico: {}", e))?;
        fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Falha ao substituir arquivo de histórico: {}", e))
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
    ///
    /// Uses `writeln!` into the pre-allocated `String` to avoid intermediate
    /// allocations per row.
    pub fn export_csv(&self) -> String {
        let header = "Data/Hora,Domínios,IPv4 Anterior,IPv4 Novo,IPv6 Anterior,IPv6 Novo,Status,Mensagem\n";
        let mut csv = String::with_capacity(self.entries.len() * 128 + header.len());
        csv.push_str(header);
        for e in &self.entries {
            // `writeln!` into String never errors.
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{},{},{}",
                Self::csv_escape(&e.timestamp),
                Self::csv_escape(&e.domains),
                Self::csv_escape(e.old_ipv4.as_deref().unwrap_or("")),
                Self::csv_escape(e.new_ipv4.as_deref().unwrap_or("")),
                Self::csv_escape(e.old_ipv6.as_deref().unwrap_or("")),
                Self::csv_escape(e.new_ipv6.as_deref().unwrap_or("")),
                if e.success { "OK" } else { "FALHA" },
                Self::csv_escape(&e.message),
            );
        }
        csv
    }

    /// Save CSV export to the config directory.
    pub fn save_csv_export(&self) -> Result<PathBuf, String> {
        let mut path = AppConfig::get_config_dir();
        path.push("duckdns_history_export.csv");
        let csv = self.export_csv();
        fs::write(&path, csv).map_err(|e| format!("Falha ao exportar CSV: {}", e))?;
        Ok(path)
    }

    /// RFC 4180-compliant CSV cell escaping.
    ///
    /// Returns a `Cow::Borrowed` (zero-copy) for the common case where no
    /// quoting is needed, and `Cow::Owned` only when the cell must be quoted.
    fn csv_escape(s: &str) -> Cow<'_, str> {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            Cow::Owned(format!("\"{}\"", s.replace('"', "\"\"")))
        } else {
            Cow::Borrowed(s)
        }
    }
}
