use rodio::source::{SineWave, Source};
use rodio::{OutputStream, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundAlert {
    Warning30s,     // Gentle two-note harmonic chime (G4 -> B4)
    ActiveSpawn,    // Warm ascending triad arpeggio (C5 -> E5 -> G5)
    BoxTimerStart,  // Short gentle rising confirmation chime (C5 -> E5, 60ms)
    BoxTimerReady,  // Distinct sweet double chime for crate cluster respawn (E5 -> G5)
    BossTimerStart, // Deep subtle resonance for Mid Boss kill registration
    BossTimerReady, // Harmonic alert for Mid Boss respawn
}

/// Dispatches harmonic, low-volume chime audio alerts on a dedicated non-blocking thread.
pub struct AudioNotifier {
    tx: Option<Sender<SoundAlert>>,
    muted: Arc<AtomicBool>,
}

impl AudioNotifier {
    pub fn new() -> Self {
        Self::with_muted(false)
    }

    pub fn with_muted(initial_muted: bool) -> Self {
        let (tx, rx) = channel::<SoundAlert>();
        let muted = Arc::new(AtomicBool::new(initial_muted));
        let muted_clone = muted.clone();

        let thread_spawn = thread::Builder::new()
            .name("audio_notifier".into())
            .spawn(move || {
                let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
                    eprintln!("[Deadlock Audio] Warning: No audio output device available.");
                    return;
                };

                while let Ok(alert) = rx.recv() {
                    if muted_clone.load(Ordering::Relaxed) {
                        continue;
                    }

                    let sink = match Sink::try_new(&stream_handle) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[Deadlock Audio] Failed to create sink: {:?}", e);
                            continue;
                        }
                    };

                    match alert {
                        SoundAlert::Warning30s => {
                            // Gentle two-note harmonic marimba/chime (G4 + B4 harmonic)
                            // Note 1: G4 (392 Hz) + quiet octave overtone
                            let n1 = SineWave::new(392.0)
                                .mix(SineWave::new(784.0).amplify(0.25))
                                .take_duration(Duration::from_millis(110))
                                .fade_in(Duration::from_millis(15))
                                .amplify(0.04);

                            let pause = SineWave::new(0.0)
                                .take_duration(Duration::from_millis(40))
                                .amplify(0.0);

                            // Note 2: B4 (493.88 Hz) + 5th overtone (D5)
                            let n2 = SineWave::new(493.88)
                                .mix(SineWave::new(587.33).amplify(0.3))
                                .take_duration(Duration::from_millis(130))
                                .fade_in(Duration::from_millis(15))
                                .amplify(0.045);

                            sink.append(n1);
                            sink.append(pause);
                            sink.append(n2);
                            sink.sleep_until_end();
                        }
                        SoundAlert::ActiveSpawn => {
                            // Warm, polite 3-note harmonic arpeggio (C5 -> E5 -> G5)
                            // 1. C5 (523.25 Hz)
                            let n1 = SineWave::new(523.25)
                                .mix(SineWave::new(1046.5).amplify(0.2))
                                .take_duration(Duration::from_millis(70))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.035);

                            // 2. E5 (659.25 Hz)
                            let n2 = SineWave::new(659.25)
                                .mix(SineWave::new(987.77).amplify(0.2))
                                .take_duration(Duration::from_millis(70))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.035);

                            // 3. G5 (783.99 Hz) with soft resonant decay
                            let n3 = SineWave::new(783.99)
                                .mix(SineWave::new(1046.5).amplify(0.35))
                                .take_duration(Duration::from_millis(160))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.04);

                            sink.append(n1);
                            sink.append(n2);
                            sink.append(n3);
                            sink.sleep_until_end();
                        }
                        SoundAlert::BoxTimerStart => {
                            // Short pleasant rising chime (C5 -> E5, 60ms) confirming key press
                            let n1 = SineWave::new(523.25)
                                .take_duration(Duration::from_millis(40))
                                .fade_in(Duration::from_millis(5))
                                .amplify(0.035);
                            let n2 = SineWave::new(659.25)
                                .take_duration(Duration::from_millis(60))
                                .fade_in(Duration::from_millis(5))
                                .amplify(0.04);
                            sink.append(n1);
                            sink.append(n2);
                            sink.sleep_until_end();
                        }
                        SoundAlert::BoxTimerReady => {
                            // Distinct double chime for ready box respawn (E5 -> G5)
                            let n1 = SineWave::new(659.25)
                                .take_duration(Duration::from_millis(80))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.04);
                            let pause = SineWave::new(0.0)
                                .take_duration(Duration::from_millis(30))
                                .amplify(0.0);
                            let n2 = SineWave::new(783.99)
                                .take_duration(Duration::from_millis(120))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.045);
                            sink.append(n1);
                            sink.append(pause);
                            sink.append(n2);
                            sink.sleep_until_end();
                        }
                        SoundAlert::BossTimerStart => {
                            // Deeper subtle tone (E4, 329.63 Hz)
                            let n1 = SineWave::new(329.63)
                                .take_duration(Duration::from_millis(120))
                                .fade_in(Duration::from_millis(15))
                                .amplify(0.04);
                            sink.append(n1);
                            sink.sleep_until_end();
                        }
                        SoundAlert::BossTimerReady => {
                            // Low-mid harmonic chime (E4 -> G4)
                            let n1 = SineWave::new(329.63)
                                .take_duration(Duration::from_millis(90))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.04);
                            let n2 = SineWave::new(392.0)
                                .take_duration(Duration::from_millis(150))
                                .fade_in(Duration::from_millis(10))
                                .amplify(0.045);
                            sink.append(n1);
                            sink.append(n2);
                            sink.sleep_until_end();
                        }
                    }
                }
            });

        if thread_spawn.is_ok() {
            Self {
                tx: Some(tx),
                muted,
            }
        } else {
            Self {
                tx: None,
                muted,
            }
        }
    }

    pub fn set_muted(&self, is_muted: bool) {
        self.muted.store(is_muted, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn toggle_mute(&self) -> bool {
        let prev = self.muted.load(Ordering::Relaxed);
        let new_val = !prev;
        self.muted.store(new_val, Ordering::Relaxed);
        new_val
    }

    pub fn play_warning(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::Warning30s);
        }
    }

    pub fn play_active(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::ActiveSpawn);
        }
    }

    pub fn play_box_start(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::BoxTimerStart);
        }
    }

    pub fn play_box_ready(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::BoxTimerReady);
        }
    }

    pub fn play_boss_start(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::BossTimerStart);
        }
    }

    pub fn play_boss_ready(&self) {
        if self.is_muted() {
            return;
        }
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::BossTimerReady);
        }
    }
}

impl Default for AudioNotifier {
    fn default() -> Self {
        Self::new()
    }
}
