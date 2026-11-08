use crate::audio::AudioNotifier;
use crate::config::OverlayConfig;
use crate::hero_tracker::{get_all_heroes, HeroSelectionSource, HeroTracker};
use crate::timers::{format_time_mmss, ClockFilter, EventKind, EventStatus, MacroEvent, MacroScheduler};
use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetWindowRect, SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN,
};

/// Triggers smooth, zero-latency native Windows window dragging via DWM using actual cursor coordinates.
pub fn start_native_window_drag() -> Option<(i32, i32)> {
    unsafe {
        let hwnd = FindWindowW(None, w!("DeadlockMacroOverlay")).ok()?;
        if hwnd.0.is_null() {
            return None;
        }

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let lparam = ((pt.y as isize) << 16) | ((pt.x as isize) & 0xFFFF);

        let _ = ReleaseCapture();
        SendMessageW(
            hwnd,
            WM_NCLBUTTONDOWN,
            WPARAM(HTCAPTION as usize),
            LPARAM(lparam),
        );

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            Some((rect.left, rect.top))
        } else {
            None
        }
    }
}

pub struct DeadlockApp {
    pub config: OverlayConfig,
    pub config_path: String,
    pub clock: ClockFilter,
    pub scheduler: MacroScheduler,
    pub audio: AudioNotifier,
    pub interactive_mode: Arc<AtomicBool>,
    pub ocr_rx: Receiver<Option<u32>>,
    pub triggered_alerts: HashSet<(EventKind, u32)>, // (kind, target_time)
    pub triggered_warnings: HashSet<(EventKind, u32)>,
    pub last_tick_instant: Instant,
    pub hero_tracker: HeroTracker,
    pub hero_menu_open: bool,
    pub last_log_poll_instant: Instant,
    pub current_window_size: Option<Vec2>,
}

impl DeadlockApp {
    pub fn new(
        config: OverlayConfig,
        config_path: String,
        interactive_mode: Arc<AtomicBool>,
        ocr_rx: Receiver<Option<u32>>,
    ) -> Self {
        let mut hero_tracker = HeroTracker::new(None);
        if !config.last_hero.is_empty() && config.last_hero != "Auto" {
            hero_tracker.set_manual_hero(&config.last_hero);
        }

        Self {
            config,
            config_path,
            clock: ClockFilter::new(),
            scheduler: MacroScheduler::default(),
            audio: AudioNotifier::new(),
            interactive_mode,
            ocr_rx,
            triggered_alerts: HashSet::new(),
            triggered_warnings: HashSet::new(),
            last_tick_instant: Instant::now(),
            hero_tracker,
            hero_menu_open: false,
            last_log_poll_instant: Instant::now(),
            current_window_size: None,
        }
    }

    /// Pulls latest OCR readings from background worker and advances local clock.
    pub fn update_timing(&mut self) {
        let mut latest_reading = None;
        while let Ok(msg) = self.ocr_rx.try_recv() {
            latest_reading = Some(msg);
        }
        if let Some(reading) = latest_reading {
            if self.clock.feed_ocr_reading(reading) {
                self.last_tick_instant = Instant::now();
            }
        }

        if self.last_tick_instant.elapsed() >= Duration::from_secs(1) {
            self.clock.tick_second();
            self.last_tick_instant = Instant::now();
        }

        // Poll Deadlock console.log every 1 second for hero spawn events
        if self.last_log_poll_instant.elapsed() >= Duration::from_secs(1) {
            self.hero_tracker.poll_log();
            self.last_log_poll_instant = Instant::now();
        }
    }

    /// Evaluates scheduled events and fires synthetic audio alerts when crossing thresholds.
    pub fn process_alerts(&mut self, events: &[MacroEvent]) {
        let now = self.clock.current_seconds();
        let mut trigger_warn = false;
        let mut trigger_active = false;

        for event in events {
            let key = (event.kind, event.target_time);
            let status = event.status(now);

            if status == EventStatus::Warning && !self.triggered_warnings.contains(&key) {
                self.triggered_warnings.insert(key);
                trigger_warn = true;
            } else if status == EventStatus::Active && !self.triggered_alerts.contains(&key) {
                self.triggered_alerts.insert(key);
                trigger_active = true;
            }
        }

        if trigger_active {
            self.audio.play_active();
        } else if trigger_warn {
            self.audio.play_warning();
        }
    }

    pub fn reset_match(&mut self) {
        self.clock.reset();
        self.scheduler.midboss.reset();
        self.triggered_alerts.clear();
        self.triggered_warnings.clear();
    }
}

impl eframe::App for DeadlockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Disable text selection globally so mouse interaction is pure controls/dragging
        ctx.style_mut(|s| s.interaction.selectable_labels = false);

        self.update_timing();

        let current_time = self.clock.current_seconds();
        let immediate_events = self.scheduler.get_immediate_next_events(current_time);
        self.process_alerts(&immediate_events);

        let is_interactive = self.interactive_mode.load(Ordering::Relaxed);

        // Close menu automatically if leaving interactive mode
        if !is_interactive && self.hero_menu_open {
            self.hero_menu_open = false;
        }

        // Fixed compact HUD geometry: 250 x 52 px (expands to 190 px only when hero menu is opened)
        let target_size = if self.hero_menu_open && is_interactive {
            Vec2::new(250.0, 190.0)
        } else {
            Vec2::new(250.0, 52.0)
        };

        if self.current_window_size != Some(target_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
            self.current_window_size = Some(target_size);
        }

        // Deadlock Noir styling: deep charcoal glassmorphism with subtle slate border (no cyan halos)
        let frame = egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(13, 17, 23, 235))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 45, 61)))
            .inner_margin(Margin::symmetric(8.0, 4.0))
            .rounding(Rounding::same(6.0));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            let hero_info = self.hero_tracker.get_hero();

            // 1. FILA SUPERIOR: Reloj | Héroe | Grip
            ui.horizontal(|ui| {
                // Clock display in amber soul gold
                let clock_text = if self.clock.is_active() {
                    format_time_mmss(current_time)
                } else {
                    "--:--".to_string()
                };
                ui.label(
                    RichText::new(clock_text)
                        .strong()
                        .color(Color32::from_rgb(245, 158, 11))
                        .size(12.5),
                );

                ui.label(RichText::new("│").color(Color32::from_rgb(55, 65, 81)).size(11.0));

                // Hero button / display
                let hero_label = if hero_info.display_name == "None" {
                    "Héroe ▾".to_string()
                } else {
                    format!("{} ▾", hero_info.display_name)
                };

                if is_interactive {
                    let btn = ui.add(
                        egui::Button::new(
                            RichText::new(hero_label)
                                .size(11.5)
                                .color(Color32::from_rgb(226, 232, 240)),
                        )
                        .frame(false),
                    );
                    if btn.clicked() {
                        self.hero_menu_open = !self.hero_menu_open;
                    }
                    if btn.hovered() {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                    }
                } else {
                    let display_str = if hero_info.display_name == "None" {
                        "—".to_string()
                    } else {
                        hero_info.display_name.clone()
                    };
                    ui.label(
                        RichText::new(display_str)
                            .size(11.5)
                            .color(Color32::from_rgb(226, 232, 240)),
                    );
                }

                // Origin tag (Auto: green dot, Manual: M tag)
                if hero_info.source == HeroSelectionSource::Auto && hero_info.display_name != "None" {
                    ui.label(RichText::new("•").small().color(Color32::from_rgb(72, 239, 128)));
                }

                // Drag handle at top right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (handle_rect, handle_response) = ui.allocate_exact_size(
                        Vec2::new(26.0, 16.0),
                        egui::Sense::click_and_drag(),
                    );

                    let is_hovered = handle_response.hovered();
                    let grip_color = if is_hovered {
                        Color32::from_rgb(226, 232, 240)
                    } else {
                        Color32::from_rgb(100, 116, 139)
                    };

                    ui.painter().text(
                        handle_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "⠿",
                        egui::FontId::proportional(13.0),
                        grip_color,
                    );

                    if handle_response.is_pointer_button_down_on() || handle_response.drag_started() {
                        if let Some((new_x, new_y)) = start_native_window_drag() {
                            self.config.win_x = new_x;
                            self.config.win_y = new_y;
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    }
                });
            });

            // 2. FILA MEDIA: Hito Macro Inmediato
            ui.add_space(1.0);
            let active_events: Vec<&MacroEvent> = immediate_events
                .iter()
                .filter(|e| e.status(current_time) == EventStatus::Active)
                .collect();

            if !active_events.is_empty() {
                let names: Vec<&str> = active_events.iter().map(|e| e.kind.display_name()).collect();
                ui.label(
                    RichText::new(format!("✦ ¡ACTIVO! {}", names.join(" + ")))
                        .color(Color32::from_rgb(72, 239, 128))
                        .strong()
                        .size(11.5),
                );
            } else if let Some(first_event) = immediate_events.first() {
                let remaining = first_event.remaining_seconds(current_time);
                let status = first_event.status(current_time);
                let names: Vec<&str> = immediate_events.iter().map(|e| e.kind.display_name()).collect();
                let label_names = names.join(" + ");

                if status == EventStatus::Warning {
                    ui.label(
                        RichText::new(format!("✦ ¡ROTACIÓN! {} en {}s", label_names, remaining))
                            .color(Color32::from_rgb(251, 146, 60))
                            .strong()
                            .size(11.5),
                    );
                } else {
                    let rem_min = remaining / 60;
                    let rem_sec = remaining % 60;
                    let target_str = format_time_mmss(first_event.target_time);

                    let boss_extra = if let Some(respawn_time) = self.scheduler.midboss.get_active_respawn(current_time) {
                        let rem = respawn_time as i32 - current_time as i32;
                        if rem > 0 {
                            format!(" │ Boss {:02}:{:02}", rem / 60, rem % 60)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    ui.label(
                        RichText::new(format!("✦ {} {} ({:02}:{:02}){}", target_str, label_names, rem_min, rem_sec, boss_extra))
                            .color(Color32::from_rgb(226, 232, 240))
                            .size(11.5),
                    );
                }
            } else {
                ui.label(
                    RichText::new("✦ Sin eventos pendientes")
                        .color(Color32::from_rgb(148, 163, 184))
                        .size(11.5),
                );
            }

            // 3. FILA INFERIOR: Micro Barra de Progreso (2px)
            if let Some(first_event) = immediate_events.first() {
                let progress = first_event.progress(current_time);
                let status = first_event.status(current_time);
                let bar_color = if status == EventStatus::Active {
                    Color32::from_rgb(72, 239, 128)
                } else if status == EventStatus::Warning {
                    Color32::from_rgb(251, 146, 60)
                } else {
                    Color32::from_rgb(245, 158, 11) // Soul gold
                };

                ui.add_space(2.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(1.0), Color32::from_rgb(22, 27, 34));
                let filled_width = (rect.width() * progress).clamp(0.0, rect.width());
                let filled_rect = egui::Rect::from_min_size(rect.min, Vec2::new(filled_width, rect.height()));
                ui.painter().rect_filled(filled_rect, Rounding::same(1.0), bar_color);
            }

            // 4. MENÚ DESPLEGABLE DE SELECCIÓN MANUAL DE HÉROE (Solo en modo interactivo)
            if self.hero_menu_open && is_interactive {
                ui.add_space(4.0);
                ui.separator();
                ui.label(
                    RichText::new("Seleccionar Héroe:")
                        .size(10.5)
                        .color(Color32::from_rgb(148, 163, 184)),
                );

                egui::ScrollArea::vertical()
                    .max_height(105.0)
                    .show(ui, |ui| {
                        // Option to reset back to Auto-detection
                        let is_auto = hero_info.source == HeroSelectionSource::Auto;
                        if ui.selectable_label(is_auto, "✦ Auto (Detectar)").clicked() {
                            self.hero_tracker.reset_to_auto();
                            self.config.last_hero.clear();
                            let _ = self.config.save_to_file(&self.config_path);
                            self.hero_menu_open = false;
                        }

                        ui.separator();

                        // Core heroes list without heap re-allocation
                        for (_id, name) in get_all_heroes() {
                            let is_selected = hero_info.source == HeroSelectionSource::Manual && hero_info.display_name == *name;
                            if ui.selectable_label(is_selected, *name).clicked() {
                                self.hero_tracker.set_manual_hero(name);
                                self.config.last_hero = name.to_string();
                                let _ = self.config.save_to_file(&self.config_path);
                                self.hero_menu_open = false;
                            }
                        }
                    });
            }
        });

        // Fast repaint in calibration mode; low CPU (200ms) in ghost mode
        if is_interactive {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}
