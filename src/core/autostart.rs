/// Windows Registry auto-start management.
///
/// Registers/unregisters the current executable in
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
/// so the app launches automatically when the user logs in.

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

const APP_NAME: &str = "DuckDNS Updater";

/// Enable or disable auto-start with Windows.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_SET_VALUE | KEY_QUERY_VALUE,
            )
            .map_err(|e| format!("Falha ao abrir chave do Registry: {}", e))?;

        if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| format!("Falha ao obter caminho do executável: {}", e))?;
            run_key
                .set_value(APP_NAME, &exe_path.to_string_lossy().to_string())
                .map_err(|e| format!("Falha ao definir valor no Registry: {}", e))?;
        } else {
            // Ignore error if the value doesn't exist
            let _ = run_key.delete_value(APP_NAME);
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err("Auto-start só é suportado no Windows.".to_string())
    }
}

/// Check whether auto-start is currently enabled.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_QUERY_VALUE,
        ) {
            let val: Result<String, _> = run_key.get_value(APP_NAME);
            return val.is_ok();
        }
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
