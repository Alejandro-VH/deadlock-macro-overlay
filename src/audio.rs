use rodio::source::{SineWave, Source};
use rodio::{OutputStream, Sink};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundAlert {
    Warning30s, // Double short beep at 880 Hz (A5)
    ActiveSpawn, // Single clear tone at 1760 Hz (A6)
}

/// Dispatches synthetic sine-wave audio alerts on a dedicated non-blocking thread.
pub struct AudioNotifier {
    tx: Option<Sender<SoundAlert>>,
}

impl AudioNotifier {
    pub fn new() -> Self {
        let (tx, rx) = channel::<SoundAlert>();

        let thread_spawn = thread::Builder::new()
            .name("audio_notifier".into())
            .spawn(move || {
                let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
                    eprintln!("[Deadlock Audio] Warning: No audio output device available.");
                    return;
                };

                while let Ok(alert) = rx.recv() {
                    let sink = match Sink::try_new(&stream_handle) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[Deadlock Audio] Failed to create sink: {:?}", e);
                            continue;
                        }
                    };

                    match alert {
                        SoundAlert::Warning30s => {
                            // Two short beeps at 880 Hz
                            let beep1 = SineWave::new(880.0)
                                .take_duration(Duration::from_millis(120))
                                .amplify(0.25);
                            let pause = SineWave::new(0.0)
                                .take_duration(Duration::from_millis(80))
                                .amplify(0.0);
                            let beep2 = SineWave::new(880.0)
                                .take_duration(Duration::from_millis(120))
                                .amplify(0.25);

                            sink.append(beep1);
                            sink.append(pause);
                            sink.append(beep2);
                            sink.sleep_until_end();
                        }
                        SoundAlert::ActiveSpawn => {
                            // High tone alert at 1760 Hz
                            let active_tone = SineWave::new(1760.0)
                                .take_duration(Duration::from_millis(250))
                                .amplify(0.25);

                            sink.append(active_tone);
                            sink.sleep_until_end();
                        }
                    }
                }
            });

        if thread_spawn.is_ok() {
            Self { tx: Some(tx) }
        } else {
            Self { tx: None }
        }
    }

    pub fn play_warning(&self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::Warning30s);
        }
    }

    pub fn play_active(&self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(SoundAlert::ActiveSpawn);
        }
    }
}

impl Default for AudioNotifier {
    fn default() -> Self {
        Self::new()
    }
}
