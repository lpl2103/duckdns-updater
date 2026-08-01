use crate::core::autostart;
use crate::core::config::AppConfig;
use crate::core::duckdns::DuckDnsService;
use crate::core::history::{HistoryEntry, UpdateHistory};
use chrono::Local;
use eframe::egui;
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

// ─── Messages for UI-driven updates ────────────────────────────────────────
pub enum UiUpdateMsg {
    Started,
    Finished {
        success: bool,
        message: String,
        ip_changed: bool,
    },
}

pub struct DuckDnsApp {
    config: AppConfig,
    /// Editable multi-domain string in the UI (comma-separated).
    domains_edit: String,
    service: Arc<DuckDnsService>,
    status_message: String,
    is_updating: bool,
    tx: Sender<UiUpdateMsg>,
    rx: Receiver<UiUpdateMsg>,
    _tray_icon: Option<TrayIcon>,
    /// Shared flag: true = window is shown on screen.
    window_visible: Arc<AtomicBool>,
    /// Set by background threads after saving new results to disk.
    config_dirty: Arc<AtomicBool>,
    /// Shared flag: network connectivity status.
    network_online: Arc<AtomicBool>,

    // ── New fields ─────────────────────────────────────────────────────────
    history: UpdateHistory,
    last_update_instant: Option<Instant>,
    show_history_panel: bool,
    show_about_dialog: bool,
    success_flash_alpha: f32,
    /// Cached autostart state from Registry.
    autostart_enabled: bool,
}

impl DuckDnsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_winui3_theme(&cc.egui_ctx);

        let config = AppConfig::load();
        let domains_edit = if config.domains.is_empty() {
            config.domain.clone()
        } else {
            config.domains.join(", ")
        };

        let service = Arc::new(DuckDnsService::new());
        let (tx, rx) = channel::<UiUpdateMsg>();
        let config_dirty = Arc::new(AtomicBool::new(false));
        let window_visible = Arc::new(AtomicBool::new(!config.start_minimized));

        let (tray_icon, open_id, force_id, exit_id) = create_tray();

        // ── Tray pump thread ────────────────────────────────────────────────
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

        // ── Auto-update background thread ───────────────────────────────────
        let service_auto = service.clone();
        let config_dirty_auto = config_dirty.clone();
        let window_visible_auto = window_visible.clone();

        thread::Builder::new()
            .name("auto-update".into())
            .spawn(move || {
                loop {
                    let cfg = AppConfig::load();
                    let interval = cfg.interval_minutes.max(1) as u64;
                    thread::sleep(Duration::from_secs(interval * 60));

                    let cfg = AppConfig::load();
                    if cfg.update_enabled
                        && !cfg.domains_csv().is_empty()
                        && !cfg.token.is_empty()
                    {
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

        // ── Network status background thread ────────────────────────────────
        let network_online = Arc::new(AtomicBool::new(true));
        let network_online_thread = network_online.clone();
        thread::Builder::new()
            .name("network-check".into())
            .spawn(move || loop {
                let is_up = check_internet_connection();
                network_online_thread.store(is_up, Ordering::Relaxed);
                thread::sleep(Duration::from_secs(10));
            })
            .expect("failed to spawn network-check thread");

        let history = UpdateHistory::load();
        let autostart_enabled = autostart::is_autostart_enabled();

        let app = Self {
            config,
            domains_edit,
            service,
            status_message: "Pronto.".to_string(),
            is_updating: false,
            tx,
            rx,
            _tray_icon: tray_icon,
            window_visible,
            config_dirty,
            network_online,
            history,
            last_update_instant: None,
            show_history_panel: false,
            show_about_dialog: false,
            success_flash_alpha: 0.0,
            autostart_enabled,
        };

        if app.config.update_enabled
            && !app.config.domains_csv().is_empty()
            && !app.config.token.is_empty()
        {
            app.trigger_ui_update();
        }

        app
    }

    /// Trigger an update from the GUI buttons.
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
                    if cfg.ipv6_enabled {
                        cfg.last_ipv6 = result.ipv6.clone();
                    }
                    let _ = cfg.save();

                    let msg = format!(
                        "Atualizado! IPv4: {} | IPv6: {}",
                        result.ipv4.as_deref().unwrap_or("N/A"),
                        result.ipv6.as_deref().unwrap_or("N/A"),
                    );
                    let _ = tx.send(UiUpdateMsg::Finished {
                        success: true,
                        message: msg,
                        ip_changed: result.ip_changed,
                    });
                }
                Err(err) => {
                    let msg = format!("Falha: {}", err);
                    let _ = tx.send(UiUpdateMsg::Finished {
                        success: false,
                        message: msg,
                        ip_changed: false,
                    });
                }
            }
        });
    }

    fn save_settings(&mut self) {
        // Sync domains from edit string
        self.config.domains = self
            .domains_edit
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // Keep legacy field in sync
        self.config.domain = self.config.domains_csv();

        // Handle autostart toggle
        if self.config.start_with_windows != self.autostart_enabled {
            match autostart::set_autostart(self.config.start_with_windows) {
                Ok(_) => {
                    self.autostart_enabled = self.config.start_with_windows;
                }
                Err(e) => {
                    self.status_message = format!("Erro ao configurar auto-start: {}", e);
                    self.config.start_with_windows = self.autostart_enabled;
                    return;
                }
            }
        }

        match self.config.save() {
            Ok(_) => {
                self.status_message = "Configurações salvas com sucesso!".to_string();
            }
            Err(e) => {
                self.status_message = format!("Erro ao salvar: {}", e);
            }
        }
    }

    /// Validate config fields; returns list of error messages.
    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let domains: Vec<&str> = self.domains_edit.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if domains.is_empty() {
            errors.push("Domínio não pode estar vazio.".into());
        }
        if self.config.token.trim().is_empty() {
            errors.push("Token não pode estar vazio.".into());
        }
        if self.config.interval_minutes < 1 {
            errors.push("Intervalo deve ser >= 1 minuto.".into());
        }
        errors
    }

    /// Record an update result in the history.
    fn record_history(&mut self, success: bool, message: &str) {
        let entry = HistoryEntry {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            domains: self.config.domains_csv(),
            old_ipv4: self.config.last_ipv4.clone(),
            new_ipv4: self.config.last_ipv4.clone(),
            old_ipv6: self.config.last_ipv6.clone(),
            new_ipv6: self.config.last_ipv6.clone(),
            success,
            message: message.to_string(),
        };
        self.history.add_entry(entry);
    }
}

impl eframe::App for DuckDnsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint every second for countdown timer & flash animation
        ctx.request_repaint_after(Duration::from_secs(1));

        // ── Reload config if a background thread saved new data ─────────────
        if self.config_dirty.swap(false, Ordering::Relaxed) {
            let old_ipv4 = self.config.last_ipv4.clone();
            let old_ipv6 = self.config.last_ipv6.clone();
            self.config = AppConfig::load();
            self.status_message = format!(
                "Atualizado! IPv4: {} | IPv6: {}",
                self.config.last_ipv4.as_deref().unwrap_or("N/A"),
                self.config.last_ipv6.as_deref().unwrap_or("N/A"),
            );
            self.last_update_instant = Some(Instant::now());
            self.success_flash_alpha = 1.0;

            // Record in history from background update
            let entry = HistoryEntry {
                timestamp: self.config.last_update.clone().unwrap_or_default(),
                domains: self.config.domains_csv(),
                old_ipv4,
                new_ipv4: self.config.last_ipv4.clone(),
                old_ipv6,
                new_ipv6: self.config.last_ipv6.clone(),
                success: true,
                message: self.status_message.clone(),
            };
            self.history.add_entry(entry);
        }

        // ── Intercept close ('X') → hide via Win32 API ─────────────────────
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            hide_main_window(&self.window_visible);
        }

        // ── Intercept OS minimise → hide via Win32 API ─────────────────────
        if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            hide_main_window(&self.window_visible);
        }

        // ── Keyboard shortcuts ─────────────────────────────────────────────
        let ctrl_held = ctx.input(|i| i.modifiers.ctrl);
        if ctrl_held && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            self.save_settings();
        }
        if ctrl_held && ctx.input(|i| i.key_pressed(egui::Key::U)) {
            self.trigger_ui_update();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            hide_main_window(&self.window_visible);
        }

        // ── Drain UI update channel ─────────────────────────────────────────
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiUpdateMsg::Started => {
                    self.is_updating = true;
                    self.status_message = "Atualizando IP no DuckDNS...".to_string();
                }
                UiUpdateMsg::Finished {
                    success,
                    message,
                    ip_changed: _,
                } => {
                    self.is_updating = false;
                    self.record_history(success, &message);
                    self.status_message = message;
                    if success {
                        self.config = AppConfig::load();
                        self.last_update_instant = Some(Instant::now());
                        self.success_flash_alpha = 1.0;
                    }
                }
            }
        }

        // ── Fade success flash ──────────────────────────────────────────────
        if self.success_flash_alpha > 0.0 {
            self.success_flash_alpha = (self.success_flash_alpha - 0.02).max(0.0);
        }

        // ── About Dialog ────────────────────────────────────────────────────
        if self.show_about_dialog {
            egui::Window::new("Sobre")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("DuckDNS Updater")
                                .size(20.0)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new("v1.1.0")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(0, 120, 212)),
                        );
                        ui.add_space(8.0);
                        ui.label("Atualizador de DNS dinâmico para DuckDNS.");
                        ui.label("Nativo, leve e seguro.");
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Desenvolvido por Leandro Pinheiro")
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new("⚡ Vibecodado com Rust + egui")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(0, 120, 212)),
                        );
                        ui.add_space(8.0);
                        ui.hyperlink_to("duckdns.org", "https://www.duckdns.org");
                        ui.add_space(8.0);
                        if ui.button("Fechar").clicked() {
                            self.show_about_dialog = false;
                        }

                    });
                });
        }

        // ── GUI ─────────────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // ── Header ──────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.heading(
                        egui::RichText::new("DuckDNS Updater")
                            .size(18.0)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("Atualizador de IP público nativo")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new("?")
                                .size(14.0)
                                .strong()
                                .color(egui::Color32::from_rgb(160, 160, 160)),
                        ).min_size(egui::vec2(28.0, 28.0)))
                        .on_hover_text("Sobre o aplicativo")
                        .clicked()
                    {
                        self.show_about_dialog = !self.show_about_dialog;
                    }
                });
            });

            ui.add_space(6.0);

            // ── Scrollable area for all content ─────────────────────────────
            egui::ScrollArea::vertical().show(ui, |ui| {

            // ── Status da rede Card ─────────────────────────────────────────
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Status da rede").strong().size(13.0));
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let is_online = self.network_online.load(Ordering::Relaxed);
                            let (status_text, dot_color) = if is_online {
                                ("Conectado", egui::Color32::from_rgb(46, 204, 113))
                            } else {
                                ("Desconectado", egui::Color32::from_rgb(231, 76, 60))
                            };

                            ui.label(
                                egui::RichText::new(status_text).color(dot_color).strong(),
                            );
                            ui.add_space(4.0);

                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(12.0, 12.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().circle_filled(
                                rect.center(),
                                6.0,
                                dot_color.linear_multiply(0.35),
                            );
                            ui.painter()
                                .circle_filled(rect.center(), 4.0, dot_color);
                        },
                    );
                });
            });

            ui.add_space(6.0);

            // ── Configuração Card ───────────────────────────────────────────
            ui.group(|ui| {
                ui.label(
                    egui::RichText::new("Configuração do Serviço")
                        .strong()
                        .size(13.0),
                );
                ui.add_space(6.0);

                let validation_errors = self.validate();

                egui::Grid::new("config_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        // ── Domínios ────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.label("Domínios:");
                            ui.label(
                                egui::RichText::new("(?)")
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 100, 100)),
                            )
                            .on_hover_text(
                                "Seus subdomínios no DuckDNS, separados por vírgula.\nExemplo: meu-servidor, outro-dominio",
                            );
                        });
                        let domain_err = validation_errors
                            .iter()
                            .any(|e| e.contains("Domínio"));
                        let domain_edit = egui::TextEdit::singleline(&mut self.domains_edit)
                            .hint_text("ex: meu-servidor, outro");
                        let resp = ui.add(domain_edit);
                        if domain_err {
                            ui.painter().rect_stroke(
                                resp.rect,
                                egui::Rounding::same(4.0),
                                egui::Stroke::new(1.5f32, egui::Color32::from_rgb(231, 76, 60)),
                            );
                        }
                        ui.end_row();

                        // ── Token ───────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.label("Token DuckDNS:");
                            ui.label(
                                egui::RichText::new("(?)")
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 100, 100)),
                            )
                            .on_hover_text(
                                "Seu token de acesso da conta DuckDNS.\nEncontre em: https://www.duckdns.org",
                            );
                        });
                        let token_err = validation_errors
                            .iter()
                            .any(|e| e.contains("Token"));
                        let token_edit =
                            egui::TextEdit::singleline(&mut self.config.token)
                                .password(true)
                                .hint_text("Token de acesso secreto");
                        let resp = ui.add(token_edit);
                        if token_err {
                            ui.painter().rect_stroke(
                                resp.rect,
                                egui::Rounding::same(4.0),
                                egui::Stroke::new(1.5f32, egui::Color32::from_rgb(231, 76, 60)),
                            );
                        }
                        ui.end_row();

                        // ── Intervalo ───────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.label("Intervalo (min):");
                            ui.label(
                                egui::RichText::new("(?)")
                                    .small()
                                    .color(egui::Color32::from_rgb(100, 100, 100)),
                            )
                            .on_hover_text(
                                "Intervalo em minutos entre atualizações automáticas.\nMínimo: 1 minuto.",
                            );
                        });
                        let interval_err = validation_errors
                            .iter()
                            .any(|e| e.contains("Intervalo"));
                        let mut interval_str =
                            self.config.interval_minutes.to_string();
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut interval_str),
                        );
                        if resp.changed() {
                            if let Ok(v) = interval_str.parse::<u32>() {
                                self.config.interval_minutes = v;
                            }
                        }
                        if interval_err {
                            ui.painter().rect_stroke(
                                resp.rect,
                                egui::Rounding::same(4.0),
                                egui::Stroke::new(1.5f32, egui::Color32::from_rgb(231, 76, 60)),
                            );
                        }
                        ui.end_row();
                    });

                // Show validation errors
                if !validation_errors.is_empty() {
                    ui.add_space(4.0);
                    for err in &validation_errors {
                        ui.label(
                            egui::RichText::new(format!("  ⚠ {}", err))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(231, 76, 60)),
                        );
                    }
                }

                ui.add_space(8.0);
                ui.checkbox(
                    &mut self.config.update_enabled,
                    "Ativar atualização automática periódica",
                );
                ui.checkbox(&mut self.config.ipv6_enabled, "Ativar IPv6");
                ui.checkbox(
                    &mut self.config.start_with_windows,
                    "Iniciar com o Windows",
                );
                ui.checkbox(
                    &mut self.config.start_minimized,
                    "Iniciar minimizado na tray",
                );
            });

            ui.add_space(6.0);

            // ── Status e Diagnóstico Card ───────────────────────────────────
            let flash_color = if self.success_flash_alpha > 0.0 {
                Some(egui::Color32::from_rgba_unmultiplied(
                    46,
                    204,
                    113,
                    (self.success_flash_alpha * 30.0) as u8,
                ))
            } else {
                None
            };

            let frame = if let Some(fc) = flash_color {
                egui::Frame::group(ui.style()).fill(fc)
            } else {
                egui::Frame::group(ui.style())
            };

            frame.show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Status e Diagnóstico")
                        .strong()
                        .size(13.0),
                );
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Última Atualização:").weak());
                    ui.label(
                        egui::RichText::new(
                            self.config
                                .last_update
                                .as_deref()
                                .unwrap_or("Nunca"),
                        )
                        .strong(),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Último IPv4:").weak());
                    ui.label(
                        egui::RichText::new(
                            self.config.last_ipv4.as_deref().unwrap_or("N/A"),
                        )
                        .monospace(),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Último IPv6:").weak());
                    ui.label(
                        egui::RichText::new(
                            self.config.last_ipv6.as_deref().unwrap_or("N/A"),
                        )
                        .monospace(),
                    );
                });

                // ── Countdown Timer ─────────────────────────────────────
                if self.config.update_enabled {
                    ui.add_space(4.0);
                    let interval_secs =
                        self.config.interval_minutes.max(1) as u64 * 60;
                    let elapsed = self
                        .last_update_instant
                        .map(|i| i.elapsed().as_secs())
                        .unwrap_or(0);
                    let remaining = interval_secs.saturating_sub(elapsed);
                    let mins = remaining / 60;
                    let secs = remaining % 60;
                    let progress = if interval_secs > 0 {
                        1.0 - (remaining as f32 / interval_secs as f32)
                    } else {
                        0.0
                    };

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Próxima atualização:")
                                .weak(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{}min {:02}s",
                                mins, secs
                            ))
                            .strong()
                            .color(egui::Color32::from_rgb(52, 152, 219)),
                        );
                    });

                    let bar =
                        egui::ProgressBar::new(progress).desired_width(ui.available_width());
                    ui.add(bar);
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if self.is_updating {
                        ui.spinner();
                    }
                    let color = if self.status_message.contains("Falha")
                        || self.status_message.contains("Erro")
                    {
                        egui::Color32::from_rgb(231, 76, 60)
                    } else if self.status_message.contains("sucesso")
                        || self.status_message.contains("Atualizado")
                    {
                        egui::Color32::from_rgb(46, 204, 113)
                    } else {
                        egui::Color32::from_rgb(52, 152, 219)
                    };
                    ui.label(
                        egui::RichText::new(&self.status_message).color(color),
                    );
                });
            });

            ui.add_space(8.0);

            // ── Action Buttons ──────────────────────────────────────────────
            ui.columns(2, |columns| {
                if columns[0]
                    .add_sized(
                        [columns[0].available_width(), 32.0],
                        egui::Button::new("💾 Salvar  (Ctrl+S)"),
                    )
                    .clicked()
                {
                    self.save_settings();
                }

                let force_btn = egui::Button::new("⚡ Atualizar  (Ctrl+U)")
                    .fill(egui::Color32::from_rgb(0, 120, 212));
                if columns[1]
                    .add_sized(
                        [columns[1].available_width(), 32.0],
                        force_btn,
                    )
                    .clicked()
                {
                    self.trigger_ui_update();
                }
            });

            ui.add_space(6.0);

            ui.columns(2, |columns| {
                if columns[0]
                    .add_sized(
                        [columns[0].available_width(), 32.0],
                        egui::Button::new("📌 Ocultar  (Esc)"),
                    )
                    .clicked()
                {
                    hide_main_window(&self.window_visible);
                }

                let hist_label = if self.show_history_panel {
                    "📋 Ocultar Histórico"
                } else {
                    "📋 Histórico"
                };
                if columns[1]
                    .add_sized(
                        [columns[1].available_width(), 32.0],
                        egui::Button::new(hist_label),
                    )
                    .clicked()
                {
                    self.show_history_panel = !self.show_history_panel;
                }
            });

            // ── History Panel (collapsible) ─────────────────────────────────
            if self.show_history_panel {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Histórico de Atualizações")
                                .strong()
                                .size(13.0),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui.small_button("Exportar CSV").clicked() {
                                    match self.history.save_csv_export() {
                                        Ok(path) => {
                                            self.status_message = format!(
                                                "CSV exportado: {}",
                                                path.display()
                                            );
                                        }
                                        Err(e) => {
                                            self.status_message = e;
                                        }
                                    }
                                }
                            },
                        );
                    });
                    ui.add_space(4.0);

                    if self.history.entries.is_empty() {
                        ui.label(
                            egui::RichText::new("Nenhuma atualização registrada.")
                                .weak()
                                .italics(),
                        );
                    } else {
                        // Show last 20 entries in reverse chronological order
                        let entries: Vec<_> = self
                            .history
                            .entries
                            .iter()
                            .rev()
                            .take(20)
                            .collect();

                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                egui::Grid::new("history_grid")
                                    .num_columns(4)
                                    .spacing([12.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        // Header
                                        ui.label(
                                            egui::RichText::new("Data/Hora")
                                                .strong()
                                                .size(11.0),
                                        );
                                        ui.label(
                                            egui::RichText::new("Domínio(s)")
                                                .strong()
                                                .size(11.0),
                                        );
                                        ui.label(
                                            egui::RichText::new("IPv4")
                                                .strong()
                                                .size(11.0),
                                        );
                                        ui.label(
                                            egui::RichText::new("Status")
                                                .strong()
                                                .size(11.0),
                                        );
                                        ui.end_row();

                                        for entry in &entries {
                                            ui.label(
                                                egui::RichText::new(
                                                    &entry.timestamp,
                                                )
                                                .size(11.0),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    &entry.domains,
                                                )
                                                .size(11.0),
                                            );
                                            ui.label(
                                                egui::RichText::new(
                                                    entry
                                                        .new_ipv4
                                                        .as_deref()
                                                        .unwrap_or("N/A"),
                                                )
                                                .size(11.0)
                                                .monospace(),
                                            );
                                            let (icon, color) = if entry.success
                                            {
                                                (
                                                    "OK",
                                                    egui::Color32::from_rgb(
                                                        46, 204, 113,
                                                    ),
                                                )
                                            } else {
                                                (
                                                    "FALHA",
                                                    egui::Color32::from_rgb(
                                                        231, 76, 60,
                                                    ),
                                                )
                                            };
                                            ui.label(
                                                egui::RichText::new(icon)
                                                    .size(11.0)
                                                    .strong()
                                                    .color(color),
                                            );
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                });
            }

            }); // end ScrollArea
        });
    }
}

// ─── Win32 window management ────────────────────────────────────────────────────

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

// ─── Background update logic ────────────────────────────────────────────────────

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
    if config.domains_csv().is_empty() || config.token.is_empty() {
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
            if cfg.ipv6_enabled {
                cfg.last_ipv6 = result.ipv6.clone();
            }
            let _ = cfg.save();
            config_dirty.store(true, Ordering::Relaxed);

            let body = format!(
                "Atualizado com sucesso!\nIPv4: {}\nIPv6: {}",
                result.ipv4.as_deref().unwrap_or("N/A"),
                result.ipv6.as_deref().unwrap_or("N/A"),
            );

            // Only notify on IP change or always when hidden
            notify_if_hidden(window_visible, "DuckDNS Updater", &body);
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
    let icon_bytes = include_bytes!("../../assets/icon.ico");
    let icon = match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            tray_icon::Icon::from_rgba(rgba.into_raw(), w, h).ok()
        }
        Err(_) => {
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
            tray_icon::Icon::from_rgba(rgba, W, H).ok()
        }
    };

    let icon = match icon {
        Some(i) => i,
        None => return (None, None, None, None),
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

fn check_internet_connection() -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout(Duration::from_secs(3))
        .build();
    agent.get("https://www.duckdns.org").call().is_ok()
        || agent.get("https://1.1.1.1").call().is_ok()
}

fn apply_winui3_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    let bg_color = egui::Color32::from_rgb(28, 28, 28);
    let card_bg = egui::Color32::from_rgb(39, 39, 39);
    let card_border = egui::Color32::from_rgb(52, 52, 52);
    let widget_bg = egui::Color32::from_rgb(45, 45, 45);
    let widget_hover = egui::Color32::from_rgb(58, 58, 58);
    let widget_active = egui::Color32::from_rgb(34, 34, 34);

    visuals.panel_fill = bg_color;
    visuals.window_fill = bg_color;
    visuals.extreme_bg_color = egui::Color32::from_rgb(22, 22, 22);

    visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
    visuals.widgets.noninteractive.bg_fill = card_bg;
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0f32, card_border);

    visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
    visuals.widgets.inactive.bg_fill = widget_bg;
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0f32, card_border);

    visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
    visuals.widgets.hovered.bg_fill = widget_hover;
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0f32, egui::Color32::from_rgb(75, 75, 75));

    visuals.widgets.active.rounding = egui::Rounding::same(6.0);
    visuals.widgets.active.bg_fill = widget_active;
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0f32, egui::Color32::from_rgb(0, 120, 212));

    ctx.set_visuals(visuals);

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "segoe_ui".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "C:\\Windows\\Fonts\\segoeui.ttf"
        )),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "segoe_ui".to_owned());

    ctx.set_fonts(fonts);
}
