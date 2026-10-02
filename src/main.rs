#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod gui;

use eframe::egui;
use gui::app::DuckDnsApp;

fn main() -> eframe::Result<()> {
    core::updater::clean_old_update_files();

    let args: Vec<String> = std::env::args().collect();

    #[cfg(target_os = "windows")]
    attach_console_if_needed();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--daemon" || a == "-d") {
        run_daemon();
        return Ok(());
    }

    let config = core::config::AppConfig::load();
    let is_minimized_arg = args.iter().any(|a| a == "--minimized");
    let start_minimized = config.start_minimized || is_minimized_arg;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DuckDNS Updater")
            .with_inner_size([490.0, 560.0])
            .with_min_inner_size([460.0, 500.0])
            .with_resizable(false)
            .with_visible(!start_minimized)
            .with_icon(load_app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "DuckDNS Updater",
        options,
        Box::new(|cc| Ok(Box::new(DuckDnsApp::new(cc)))),
    )
}

fn print_help() {
    println!("DuckDNS Updater v{}", env!("CARGO_PKG_VERSION"));
    println!("Atualizador nativo de IP publico para DuckDNS\n");
    println!("Uso: duckdns-updater.exe [OPCOES]\n");
    println!("Opcoes:");
    println!("  --daemon, -d      Executa em modo daemon continuo sem GUI (servidor / background)");
    println!("  --minimized       Inicia o aplicativo minimizado na bandeja do sistema");
    println!("  --cleanup-old     Remove arquivos temporarios .old de atualizacoes anteriores");
    println!("  --help, -h        Exibe esta mensagem de ajuda");
}

fn run_daemon() {
    println!("=======================================================");
    println!("  DuckDNS Updater v{} [Modo Daemon]", env!("CARGO_PKG_VERSION"));
    println!("=======================================================");
    println!("Iniciando servico em segundo plano...");

    let service = core::duckdns::DuckDnsService::new();

    loop {
        let config = core::config::AppConfig::load();
        let domains = config.domains_csv().into_owned();

        if domains.is_empty() || config.token.is_empty() {
            eprintln!(
                "[{}] AVISO: Dominio ou token nao configurados. Configure o arquivo duckdns_config.json.",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            );
        } else if !config.update_enabled {
            println!(
                "[{}] Atualizacao automatica pausada na configuracao.",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            );
        } else {
            let old_ipv4 = config.last_ipv4.clone();
            let old_ipv6 = config.last_ipv6.clone();
            println!(
                "[{}] Verificando IP publico para: {}...",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                domains
            );

            match service.update(&config) {
                Ok(result) => {
                    let mut cfg = config.clone();
                    let now_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    cfg.last_update = Some(now_str.clone());
                    cfg.last_ipv4 = result.ipv4.clone();
                    if cfg.ipv6_enabled {
                        cfg.last_ipv6 = result.ipv6.clone();
                    }
                    let _ = cfg.save();

                    println!(
                        "[{}] SUCESSO: IPv4 = {} | IPv6 = {} | Mudou? {}",
                        now_str,
                        result.ipv4.as_deref().unwrap_or("N/A"),
                        result.ipv6.as_deref().unwrap_or("N/A"),
                        if result.ip_changed { "Sim" } else { "Nao" }
                    );

                    let mut history = core::history::UpdateHistory::load();
                    history.add_entry(core::history::HistoryEntry {
                        timestamp: now_str.clone(),
                        domains: domains.clone(),
                        old_ipv4: old_ipv4.clone(),
                        new_ipv4: result.ipv4.clone(),
                        old_ipv6: old_ipv6.clone(),
                        new_ipv6: result.ipv6.clone(),
                        success: true,
                        message: "Atualizacao concluida com sucesso via daemon".to_string(),
                    });

                    if !config.notify_on_change_only || result.ip_changed {
                        let event = core::webhook::WebhookEvent {
                            domains: domains.clone(),
                            old_ipv4,
                            new_ipv4: result.ipv4,
                            old_ipv6,
                            new_ipv6: result.ipv6,
                            success: true,
                            message: "IP atualizado com sucesso pelo modo daemon".to_string(),
                            timestamp: now_str,
                        };
                        core::webhook::send_notifications(&config, &event);
                    }
                }
                Err(err) => {
                    let now_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    eprintln!("[{}] FALHA: {}", now_str, err);

                    let mut history = core::history::UpdateHistory::load();
                    history.add_entry(core::history::HistoryEntry {
                        timestamp: now_str.clone(),
                        domains: domains.clone(),
                        old_ipv4: old_ipv4.clone(),
                        new_ipv4: None,
                        old_ipv6: old_ipv6.clone(),
                        new_ipv6: None,
                        success: false,
                        message: format!("Falha no modo daemon: {}", err),
                    });

                    let event = core::webhook::WebhookEvent {
                        domains,
                        old_ipv4,
                        new_ipv4: None,
                        old_ipv6,
                        new_ipv6: None,
                        success: false,
                        message: format!("Falha no modo daemon: {}", err),
                        timestamp: now_str,
                    };
                    core::webhook::send_notifications(&config, &event);
                }
            }
        }

        let fresh_cfg = core::config::AppConfig::load();
        let sleep_secs = fresh_cfg.interval_minutes.max(1) as u64 * 60;
        println!(
            "[{}] Proxima verificacao em {} minuto(s)...",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            fresh_cfg.interval_minutes.max(1)
        );
        std::thread::sleep(std::time::Duration::from_secs(sleep_secs));
    }
}

#[cfg(target_os = "windows")]
fn attach_console_if_needed() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn SetStdHandle(nStdHandle: u32, hHandle: isize) -> i32;
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *const std::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11
    const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;  // -12
    const GENERIC_READ: u32 = 0x80000000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;

    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            let conout_name: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
            let handle = CreateFileW(
                conout_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            );
            if handle != -1 {
                SetStdHandle(STD_OUTPUT_HANDLE, handle);
                SetStdHandle(STD_ERROR_HANDLE, handle);
            }
        }
    }
}

fn load_app_icon() -> egui::IconData {
    const RAW_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon_32.rgba"));
    egui::IconData {
        rgba: RAW_RGBA.to_vec(),
        width: 32,
        height: 32,
    }
}
