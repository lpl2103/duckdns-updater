use crate::core::config::AppConfig;
use crate::core::duckdns::DuckDnsService;
use chrono::Local;
use eframe::egui;
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

// ─── Messages for UI-driven updates (window visible, event loop running) ────
pub enum UiUpdateMsg {
    Started,
    Finished { success: bool, message: String },
}

pub struct DuckDnsApp {
    config: AppConfig,
    service: Arc<DuckDnsService>,
    status_message: String,
    is_updating: bool,
    tx: Sender<UiUpdateMsg>,
    rx: Receiver<UiUpdateMsg>,
    _tray_icon: Option<TrayIcon>,
    /// Shared flag: true = window is shown on screen, false = hidden in tray.
    window_visible: Arc<AtomicBool>,
    /// Set by background threads after saving new results to disk.
    config_dirty: Arc<AtomicBool>,
}

impl DuckDnsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let config = AppConfig::load();
        let service = Arc::new(DuckDnsService::new());
        let (tx, rx) = channel::<UiUpdateMsg>();
        let config_dirty = Arc::new(AtomicBool::new(false));
        let window_visible = Arc::new(AtomicBool::new(true));

        let (tray_icon, open_id, force_id, exit_id) = create_tray();

        // ── Tray pump thread ────────────────────────────────────────────────────
        let service_tray = service.clone();
        let config_dirty_tray = config_dirty.clone();
        let window_visible_tray = window_visible.clone();

        thread::Builder::new()
            .name("tray-pump".into())
            .spawn(move || {
                loop {
                    #[cfg(target_os = "windows")]
                    unsafe {
                        use winapi::um::winuser::{
                            DispatchMessageW, PeekMessageW, TranslateMessage, PM_REMOVE,
                        };
                        let mut msg = std::mem::zeroed();
                        while PeekMessageW(
                            &mut msg,
                            std::ptr::null_mut(),
                            0,
                            0,
                            PM_REMOVE,
                        ) != 0
                        {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }

                    while let Ok(ev) = MenuEvent::receiver().try_recv() {
                        if Some(&ev.id) == open_id.as_ref() {
                            show_main_window(&window_visible_tray);
                        } else if Some(&ev.id) == force_id.as_ref() {
                            let svc = service_tray.clone();
                            let dirty = config_dirty_tray.clone();
                            let vis = window_visible_tray.clone();
                            thread::spawn(move || {
                                run_tray_update(&svc, &dirty, &vis);
                            });
                        } else if Some(&ev.id) == exit_id.as_ref() {
                            std::process::exit(0);
                        }
                    }

                    thread::sleep(Duration::from_millis(50));
                }
            })
            .expect("failed to spawn tray-pump thread");

        // ── Auto-update background thread ───────────────────────────────────────
        let service_auto = service.clone();
        let config_dirty_auto = config_dirty.clone();
        let window_visible_auto = window_visible.clone();

        thread::Builder::new()
            .name("auto-update".into())
            .spawn(move || {
                loop {
                    let cfg = AppConfig::load();
                    let interval = cfg.interval_minutes.max(5) as u64;
                    thread::sleep(Duration::from_secs(interval * 60));

                    let cfg = AppConfig::load();
                    if cfg.update_enabled && !cfg.domain.is_empty() && !cfg.token.is_empty() {
                        run_background_update(
                            &service_auto,
                            &cfg,
                            &config_dirty_auto,
                            &window_visible_auto,
                        );
                    }
                }
            })
            .expect("failed to spawn auto-update thread");

        let app = Self {
            config,
            service,
            status_message: "Pronto.".to_string(),
            is_updating: false,
            tx,
            rx,
            _tray_icon: tray_icon,
            window_visible,
            config_dirty,
        };

        if app.config.update_enabled
            && !app.config.domain.is_empty()
            && !app.config.token.is_empty()
        {
            app.trigger_ui_update();
        }

        app
    }

    /// Trigger an update from the GUI buttons. No notifications (window is open).
    fn trigger_ui_update(&self) {
        if self.is_updating {
            return;
        }

        let config = self.config.clone();
        let service = self.service.clone();
        let tx = self.tx.clone();

        let _ = tx.send(UiUpdateMsg::Started);

        thread::spawn(move || {
            match service.update(&config) {
                Ok(result) => {
                    let mut cfg = config;
                    cfg.last_update =
                        Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                    cfg.last_ipv4 = result.ipv4.clone();
                    cfg.last_ipv6 = result.ipv6.clone();
                    let _ = cfg.save();

                    let msg = format!(
                        "Atualizado! IPv4: {} | IPv6: {}",
                        result.ipv4.as_deref().unwrap_or("N/A"),
                        result.ipv6.as_deref().unwrap_or("N/A"),
                    );
                    let _ = tx.send(UiUpdateMsg::Finished {
                        success: true,
                        message: msg,
                    });
                }
                Err(err) => {
                    let msg = format!("Falha: {}", err);
                    let _ = tx.send(UiUpdateMsg::Finished {
                        success: false,
                        message: msg,
                    });
                }
            }
        });
    }

    fn save_settings(&mut self) {
        match self.config.save() {
            Ok(_) => {
                self.status_message = "Configurações salvas com sucesso!".to_string();
            }
            Err(e) => {
                self.status_message = format!("Erro ao salvar: {}", e);
            }
        }
    }
}

impl eframe::App for DuckDnsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Reload config if a background thread saved new data ─────────────────
        if self.config_dirty.swap(false, Ordering::Relaxed) {
            self.config = AppConfig::load();
            self.status_message = format!(
                "Atualizado! IPv4: {} | IPv6: {}",
                self.config.last_ipv4.as_deref().unwrap_or("N/A"),
                self.config.last_ipv6.as_deref().unwrap_or("N/A"),
            );
        }

        // ── Intercept close ('X') → hide via Win32 API ─────────────────────────
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            hide_main_window(&self.window_visible);
        }

        // ── Intercept OS minimise → hide via Win32 API ─────────────────────────
        if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            hide_main_window(&self.window_visible);
        }

        // ── Drain UI update channel ─────────────────────────────────────────────
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiUpdateMsg::Started => {
                    self.is_updating = true;
                    self.status_message = "Atualizando IP no DuckDNS...".to_string();
                }
                UiUpdateMsg::Finished { success, message } => {
                    self.is_updating = false;
                    self.status_message = message;
                    if success {
                        self.config = AppConfig::load();
                    }
                }
            }
        }

        // ── GUI ─────────────────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.heading("DuckDNS Dynamic DNS Updater");
                ui.label(
                    egui::RichText::new(
                        "Atualizador de IP público nativo e multiplataforma",
                    )
                    .small()
                    .weak(),
                );
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            egui::Grid::new("config_grid")
                .num_columns(2)
                .spacing([12.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Domínio:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.domain)
                            .hint_text("ex: meu-servidor"),
                    );
                    ui.end_row();

                    ui.label("Token DuckDNS:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.config.token)
                            .password(true)
                            .hint_text("Token de acesso secreto"),
                    );
                    ui.end_row();

                    ui.label("Intervalo (minutos):");
                    let mut interval_str = self.config.interval_minutes.to_string();
                    if ui
                        .add(egui::TextEdit::singleline(&mut interval_str))
                        .changed()
                    {
                        if let Ok(v) = interval_str.parse::<u32>() {
                            self.config.interval_minutes = v;
                        }
                    }
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.checkbox(
                &mut self.config.update_enabled,
                "Ativar atualização automática periódica",
            );

            ui.add_space(15.0);
            ui.group(|ui| {
                ui.heading("Status e Diagnóstico");
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.label("Última Atualização:");
                    ui.label(
                        egui::RichText::new(
                            self.config.last_update.as_deref().unwrap_or("Nunca"),
                        )
                        .strong(),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Último IPv4:");
                    ui.label(
                        egui::RichText::new(
                            self.config.last_ipv4.as_deref().unwrap_or("N/A"),
                        )
                        .monospace(),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Último IPv6:");
                    ui.label(
                        egui::RichText::new(
                            self.config.last_ipv6.as_deref().unwrap_or("N/A"),
                        )
                        .monospace(),
                    );
                });

                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if self.is_updating {
                        ui.spinner();
                    }
                    let color = if self.status_message.contains("Falha")
                        || self.status_message.contains("Erro")
                    {
                        egui::Color32::RED
                    } else if self.status_message.contains("sucesso")
                        || self.status_message.contains("Atualizado")
                    {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::LIGHT_BLUE
                    };
                    ui.label(egui::RichText::new(&self.status_message).color(color));
                });
            });

            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized([110.0, 30.0], egui::Button::new("💾 Salvar Settings"))
                    .clicked()
                {
                    self.save_settings();
                }

                if ui
                    .add_sized(
                        [130.0, 30.0],
                        egui::Button::new("⚡ Forçar Atualização"),
                    )
                    .clicked()
                {
                    self.trigger_ui_update();
                }

                if ui
                    .add_sized(
                        [110.0, 30.0],
                        egui::Button::new("📌 Ocultar Tray"),
                    )
                    .clicked()
                {
                    hide_main_window(&self.window_visible);
                }

                if ui
                    .add_sized(
                        [90.0, 30.0],
                        egui::Button::new("❌ Fechar App"),
                    )
                    .clicked()
                {
                    std::process::exit(0);
                }
            });
        });
    }
}

// ─── Win32 window management ────────────────────────────────────────────────────
// Both show and hide use the Win32 API directly to avoid desynchronising
// eframe/winit's internal visibility state.

/// Hide the main window using the Win32 API.
fn hide_main_window(visible_flag: &AtomicBool) {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::um::winuser::{FindWindowW, ShowWindow, SW_HIDE};
        let title: Vec<u16> = "DuckDNS Updater\0".encode_utf16().collect();
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
    visible_flag.store(false, Ordering::Relaxed);
}

/// Restore and focus the main window using the Win32 API.
fn show_main_window(visible_flag: &AtomicBool) {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::um::winuser::{
            FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };
        let title: Vec<u16> = "DuckDNS Updater\0".encode_utf16().collect();
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
    visible_flag.store(true, Ordering::Relaxed);
}

// ─── Background update logic (independent of eframe) ───────────────────────────

fn run_tray_update(
    service: &DuckDnsService,
    config_dirty: &AtomicBool,
    window_visible: &AtomicBool,
) {
    notify_if_hidden(
        window_visible,
        "DuckDNS Updater",
        "Atualização manual iniciada...",
    );

    let config = AppConfig::load();
    if config.domain.is_empty() || config.token.is_empty() {
        notify_if_hidden(
            window_visible,
            "DuckDNS Updater",
            "Configure domínio e token primeiro.",
        );
        return;
    }

    run_background_update(service, &config, config_dirty, window_visible);
}

fn run_background_update(
    service: &DuckDnsService,
    config: &AppConfig,
    config_dirty: &AtomicBool,
    window_visible: &AtomicBool,
) {
    match service.update(config) {
        Ok(result) => {
            let mut cfg = config.clone();
            cfg.last_update =
                Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            cfg.last_ipv4 = result.ipv4.clone();
            cfg.last_ipv6 = result.ipv6.clone();
            let _ = cfg.save();
            config_dirty.store(true, Ordering::Relaxed);

            notify_if_hidden(
                window_visible,
                "DuckDNS Updater",
                &format!(
                    "Atualizado com sucesso!\nIPv4: {}\nIPv6: {}",
                    result.ipv4.as_deref().unwrap_or("N/A"),
                    result.ipv6.as_deref().unwrap_or("N/A"),
                ),
            );
        }
        Err(err) => {
            notify_if_hidden(
                window_visible,
                "DuckDNS Updater - Erro",
                &format!("Falha na atualização: {}", err),
            );
        }
    }
}

/// Show a desktop notification ONLY when the window is hidden in the tray.
fn notify_if_hidden(window_visible: &AtomicBool, title: &str, body: &str) {
    if !window_visible.load(Ordering::Relaxed) {
        let _ = Notification::new()
            .summary(title)
            .body(body)
            .timeout(notify_rust::Timeout::Milliseconds(4000))
            .show();
    }
}

// ─── Tray icon creation ────────────────────────────────────────────────────────

fn create_tray() -> (
    Option<TrayIcon>,
    Option<tray_icon::menu::MenuId>,
    Option<tray_icon::menu::MenuId>,
    Option<tray_icon::menu::MenuId>,
) {
    const W: u32 = 32;
    const H: u32 = 32;
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            if dx * dx + dy * dy <= 14.0 * 14.0 {
                rgba.extend_from_slice(&[20, 140, 220, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    let icon = match tray_icon::Icon::from_rgba(rgba, W, H) {
        Ok(i) => i,
        Err(_) => return (None, None, None, None),
    };

    let open_item = MenuItem::new("Abrir Configurações", true, None);
    let force_item = MenuItem::new("Forçar Atualização", true, None);
    let exit_item = MenuItem::new("Sair", true, None);

    let open_id = open_item.id().clone();
    let force_id = force_item.id().clone();
    let exit_id = exit_item.id().clone();

    let tray_menu = Menu::new();
    let _ = tray_menu.append(&open_item);
    let _ = tray_menu.append(&force_item);
    let _ = tray_menu.append(&exit_item);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("DuckDNS Updater")
        .with_icon(icon)
        .build()
        .ok();

    (tray_icon, Some(open_id), Some(force_id), Some(exit_id))
}
