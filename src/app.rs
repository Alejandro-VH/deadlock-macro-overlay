use crate::audio::AudioNotifier;
use crate::config::OverlayConfig;
use crate::timers::{format_time_mmss, ClockFilter, EventKind, EventStatus, MacroEvent, MacroScheduler};
use eframe::egui::{self, Color32, Margin, Rounding, Stroke, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
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
    pub last_tick_instant: std::time::Instant,
}

impl DeadlockApp {
    pub fn new(
        config: OverlayConfig,
        config_path: String,
        interactive_mode: Arc<AtomicBool>,
        ocr_rx: Receiver<Option<u32>>,
    ) -> Self {
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
            last_tick_instant: std::time::Instant::now(),
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
                self.last_tick_instant = std::time::Instant::now();
            }
        }

        if self.last_tick_instant.elapsed() >= std::time::Duration::from_secs(1) {
            self.clock.tick_second();
            self.last_tick_instant = std::time::Instant::now();
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
        // Disable text selection globally so dragging is never hijacked by text selection
        ctx.style_mut(|s| s.interaction.selectable_labels = false);

        self.update_timing();

        let current_time = self.clock.current_seconds();
        // Get ONLY the immediate next events (sharing earliest target time)
        let immediate_events = self.scheduler.get_immediate_next_events(current_time);
        self.process_alerts(&immediate_events);

        let is_interactive = self.interactive_mode.load(Ordering::Relaxed);

        // Adjust window size dynamically: ultra-compact HUD
        if is_interactive {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(260.0, 92.0)));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(250.0, 68.0)));
        }

        // Visual frame styling
        let frame = if is_interactive {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(16, 20, 26, 240))
                .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0, 210, 255))) // Cyan border indicating calibration
                .inner_margin(Margin::symmetric(10.0, 7.0))
                .rounding(Rounding::same(8.0))
        } else {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(10, 12, 16, 195))
                .stroke(Stroke::NONE)
                .inner_margin(Margin::symmetric(10.0, 7.0))
                .rounding(Rounding::same(6.0))
        };

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if is_interactive {
                // Dedicated, 100% reliable drag handle bar ("Zona fija")
                ui.horizontal(|ui| {
                    let handle_width = (ui.available_width() - 52.0).max(60.0);
                    let (handle_rect, handle_response) = ui.allocate_exact_size(
                        Vec2::new(handle_width, 18.0),
                        egui::Sense::click_and_drag(),
                    );

                    let is_hovered = handle_response.hovered();
                    let bg_color = if is_hovered {
                        Color32::from_rgba_premultiplied(0, 170, 235, 100)
                    } else {
                        Color32::from_rgba_premultiplied(32, 44, 62, 160)
                    };

                    ui.painter().rect_filled(handle_rect, Rounding::same(4.0), bg_color);
                    ui.painter().rect_stroke(handle_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(0, 190, 255)));

                    ui.painter().text(
                        handle_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "⠿ ARRASTRAR AQUÍ ⠿",
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );

                    if handle_response.is_pointer_button_down_on() || handle_response.drag_started() {
                        if let Some((new_x, new_y)) = start_native_window_drag() {
                            self.config.win_x = new_x;
                            self.config.win_y = new_y;
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Reset").clicked() {
                            self.config = OverlayConfig::default_for_resolution(1920, 1080);
                            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                                self.config.win_x as f32,
                                self.config.win_y as f32,
                            )));
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    });
                });
                ui.separator();
            }

            // 1. BANNER DEL SIGUIENTE EVENTO INMEDIATO (Único objetivo en foco)
            let active_events: Vec<&MacroEvent> = immediate_events
                .iter()
                .filter(|e| e.status(current_time) == EventStatus::Active)
                .collect();

            if !active_events.is_empty() {
                let names: Vec<&str> = active_events.iter().map(|e| e.kind.display_name()).collect();
                let banner_text = format!("¡ACTIVO AHORA! {}", names.join(" + "));
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(banner_text)
                            .color(Color32::from_rgb(72, 239, 128)) // Lime green
                            .strong()
                            .size(13.5),
                    );
                });
            } else if let Some(first_event) = immediate_events.first() {
                let remaining = first_event.remaining_seconds(current_time);
                let status = first_event.status(current_time);
                let names: Vec<&str> = immediate_events.iter().map(|e| e.kind.display_name()).collect();
                let label_names = names.join(" + ");

                ui.vertical_centered(|ui| {
                    if status == EventStatus::Warning {
                        ui.label(
                            egui::RichText::new(format!("¡ROTACIÓN! {} en {}s", label_names, remaining))
                                .color(Color32::from_rgb(251, 146, 60)) // Orange warning
                                .strong()
                                .size(13.0),
                        );
                    } else {
                        let rem_min = remaining / 60;
                        let rem_sec = remaining % 60;
                        let target_str = format_time_mmss(first_event.target_time);
                        ui.label(
                            egui::RichText::new(format!("{} [{}] en {:02}:{:02}", label_names, target_str, rem_min, rem_sec))
                                .color(Color32::WHITE)
                                .strong()
                                .size(12.5),
                        );
                    }
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Sin eventos pendientes").color(Color32::GRAY).small());
                });
            }

            // 2. BARRA DE PROGRESO VISUAL DINÁMICA
            if let Some(first_event) = immediate_events.first() {
                let progress = first_event.progress(current_time);
                let status = first_event.status(current_time);
                let bar_color = if status == EventStatus::Active {
                    Color32::from_rgb(72, 239, 128) // Lime green
                } else if status == EventStatus::Warning {
                    Color32::from_rgb(251, 146, 60) // Orange
                } else {
                    Color32::from_rgb(0, 190, 255) // Cyan
                };

                ui.add_space(2.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.5), egui::Sense::hover());
                ui.painter().rect_filled(rect, Rounding::same(1.25), Color32::from_rgba_premultiplied(45, 55, 70, 140));
                let filled_width = (rect.width() * progress).clamp(0.0, rect.width());
                let filled_rect = egui::Rect::from_min_size(rect.min, Vec2::new(filled_width, rect.height()));
                ui.painter().rect_filled(filled_rect, Rounding::same(1.25), bar_color);
            }

            ui.add_space(2.0);

            // 3. BARRA DE ESTADO INFERIOR
            ui.horizontal(|ui| {
                let clock_display = if self.clock.is_active() {
                    format!("Reloj: {} [Auto]", format_time_mmss(current_time))
                } else {
                    "Reloj: En espera [OCR]".to_string()
                };
                ui.label(egui::RichText::new(clock_display).small().color(Color32::LIGHT_GRAY));

                // Display active MidBoss respawn countdown if tracking
                if let Some(respawn_time) = self.scheduler.midboss.get_active_respawn(current_time) {
                    let rem = respawn_time as i32 - current_time as i32;
                    if rem > 0 {
                        let text = format!("| MidBoss: {:02}:{:02}", rem / 60, rem % 60);
                        ui.label(egui::RichText::new(text).small().color(Color32::YELLOW));
                    }
                }
            });
        });

        // Fast responsive repaint in calibration mode; low CPU (200ms) in ghost mode
        if is_interactive {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}
