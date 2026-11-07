use crate::audio::AudioNotifier;
use crate::config::OverlayConfig;
use crate::timers::{format_time_mmss, ClockFilter, EventKind, EventStatus, MacroEvent, MacroScheduler};
use eframe::egui::{self, Color32, Margin, Rounding, Stroke, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// Tabs to keep the architecture ready for Phase 2 without rewriting the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    TimersOverlay,
    BuildsCounter, // Reserved for Phase 2
}

pub struct DeadlockApp {
    pub tab: AppTab,
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
            tab: AppTab::TimersOverlay,
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
        // Drain newest OCR reading
        let mut latest_reading = None;
        while let Ok(msg) = self.ocr_rx.try_recv() {
            latest_reading = Some(msg);
        }
        if let Some(reading) = latest_reading {
            if self.clock.feed_ocr_reading(reading) {
                // Reset tick timer when OCR successfully updates the clock to avoid double increment
                self.last_tick_instant = std::time::Instant::now();
            }
        }

        // Tick local clock once per second if active
        if self.last_tick_instant.elapsed() >= std::time::Duration::from_secs(1) {
            self.clock.tick_second();
            self.last_tick_instant = std::time::Instant::now();
        }
    }

    /// Evaluates scheduled events and fires synthetic audio alerts when crossing thresholds.
    /// Deduplicates multiple simultaneous events to avoid consecutive audio stutter.
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
        self.update_timing();

        let current_time = self.clock.current_seconds();
        let events = self.scheduler.calculate_events(current_time);
        self.process_alerts(&events);

        let is_interactive = self.interactive_mode.load(Ordering::Relaxed);

        // Dynamically adjust viewport size between compact HUD and full calibration controls
        if is_interactive {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(310.0, 340.0)));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(300.0, 155.0)));
        }

        // Styling based on interactive state
        let frame = if is_interactive {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(18, 20, 24, 240))
                .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0, 210, 255))) // Cyan border for calibration
                .inner_margin(Margin::same(8.0))
                .rounding(Rounding::same(8.0))
        } else {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(10, 12, 16, 190))
                .stroke(Stroke::NONE)
                .inner_margin(Margin::same(8.0))
                .rounding(Rounding::same(6.0))
        };

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if is_interactive {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Title Bar with Drag handle
                    ui.horizontal(|ui| {
                        let drag_btn = ui.button("✥ Arrastrar Overlay");
                        if drag_btn.drag_started() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }

                        ui.selectable_value(&mut self.tab, AppTab::TimersOverlay, "⏱ Timers");
                        ui.selectable_value(&mut self.tab, AppTab::BuildsCounter, "🛡 Builds");
                    });
                    ui.separator();

                    match self.tab {
                        AppTab::TimersOverlay => {
                            self.render_timers_view(ui, current_time, &events, true);
                        }
                        AppTab::BuildsCounter => {
                            ui.vertical_centered(|ui| {
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Módulo de Builds y Counters (Fase 2)").strong());
                                ui.label(egui::RichText::new("Conexión con api.deadlock-api.com").small().color(Color32::GRAY));
                            });
                        }
                    }
                });
            } else {
                self.render_timers_view(ui, current_time, &events, false);
            }
        });

        // Request repaint every 200ms to keep timer smoothly updated with minimal CPU
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

impl DeadlockApp {
    fn render_timers_view(
        &mut self,
        ui: &mut egui::Ui,
        current_time: u32,
        events: &[MacroEvent],
        is_interactive: bool,
    ) {
        // 1. BANNER "PRÓXIMO EVENTO"
        let active_events: Vec<&MacroEvent> = events
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
                        .size(15.0),
                );
            });
        } else if let Some(next_event) = events.first() {
            let remaining = next_event.remaining_seconds(current_time);
            let status = next_event.status(current_time);

            // Group simultaneous closest events
            let simultaneous: Vec<&str> = events
                .iter()
                .filter(|e| e.target_time == next_event.target_time)
                .map(|e| e.kind.display_name())
                .collect();
            let label_names = simultaneous.join(" + ");

            ui.vertical_centered(|ui| {
                if status == EventStatus::Warning {
                    ui.label(
                        egui::RichText::new(format!("¡ROTACIÓN! {} en {}s", label_names, remaining))
                            .color(Color32::from_rgb(251, 146, 60)) // Orange warning
                            .strong()
                            .size(14.0),
                    );
                } else {
                    let formatted_target = format_time_mmss(next_event.target_time);
                    ui.label(
                        egui::RichText::new(format!("PRÓXIMO: {} [{}]", label_names, formatted_target))
                            .color(Color32::WHITE)
                            .strong()
                            .size(13.5),
                    );
                }
            });
        }

        ui.add_space(4.0);
        ui.separator();

        // 2. COLA DE PRÓXIMOS EVENTOS (Siguientes 2-3)
        ui.vertical(|ui| {
            let upcoming_slice = if events.len() > 1 { &events[1..events.len().min(4)] } else { &[] };
            for event in upcoming_slice {
                let rem = event.remaining_seconds(current_time);
                let target = format_time_mmss(event.target_time);
                let text = format!("• {:<18} ({}) en {:02}:{:02}", event.kind.display_name(), target, rem / 60, rem % 60);
                ui.label(egui::RichText::new(text).size(11.5).color(Color32::from_rgb(203, 213, 225)));
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // 3. BARRA DE ESTADO INFERIOR
        ui.horizontal(|ui| {
            let clock_display = if self.clock.is_active() {
                format!("Reloj: {} [Auto]", format_time_mmss(current_time))
            } else {
                "Reloj: En espera [OCR]".to_string()
            };
            ui.label(egui::RichText::new(clock_display).small().color(Color32::LIGHT_GRAY));

            if self.scheduler.midboss.kills > 0 {
                ui.label(
                    egui::RichText::new(format!("| MidBoss Kills: {}", self.scheduler.midboss.kills))
                        .small()
                        .color(Color32::YELLOW),
                );
            }
        });

        // 4. MODO CALIBRACIÓN (Controles mostrados únicamente en F10)
        if is_interactive {
            ui.add_space(6.0);
            ui.separator();

            // Manual clock overrides in case of OCR fallback
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Reloj Manual:").small().strong());
                let toggle_label = if self.clock.is_active() { "⏸ Pausa" } else { "▶ Iniciar" };
                if ui.button(toggle_label).clicked() {
                    self.clock.toggle_active();
                }
                if ui.button("-5s").clicked() {
                    self.clock.adjust_seconds(-5);
                }
                if ui.button("+5s").clicked() {
                    self.clock.adjust_seconds(5);
                }
            });

            ui.add_space(4.0);
            ui.label(egui::RichText::new("Coordenadas ROI de Pantalla:").strong().small());

            egui::Grid::new("roi_grid").num_columns(2).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                ui.label("Posición X:");
                ui.add(egui::DragValue::new(&mut self.config.roi_x).speed(1.0));
                ui.end_row();

                ui.label("Posición Y:");
                ui.add(egui::DragValue::new(&mut self.config.roi_y).speed(1.0));
                ui.end_row();

                ui.label("Ancho (W):");
                ui.add(egui::DragValue::new(&mut self.config.roi_w).speed(1.0));
                ui.end_row();

                ui.label("Alto (H):");
                ui.add(egui::DragValue::new(&mut self.config.roi_h).speed(1.0));
                ui.end_row();
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Guardar").clicked() {
                    let _ = self.config.save_to_file(&self.config_path);
                }
                if ui.button("Defaults").clicked() {
                    self.config = OverlayConfig::default_for_resolution(1920, 1080);
                    let _ = self.config.save_to_file(&self.config_path);
                }
                if ui.button("Reset Partida").clicked() {
                    self.reset_match();
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Atajos: [F10] Passthrough | [Shift+F10] Reset | [F9] MidBoss | [Shift+F9] Deshacer")
                    .small()
                    .color(Color32::GRAY),
            );
        }
    }
}
