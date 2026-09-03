use rodio::{Decoder, OutputStream, Sink};
use cpal::traits::{HostTrait, DeviceTrait};
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

pub enum AudioCommand {
    Play(String),
    Pause,
    Resume,
    SetVolume(f32),
    Seek(f64),
}

struct AudioOutput {
    _stream: OutputStream,
    sink: Sink,
}

pub struct AudioSystem {
    command_tx: Sender<AudioCommand>,
}

fn get_default_device_name() -> Option<String> {
    let host = cpal::default_host();
    host.default_output_device()
        .and_then(|d| d.name().ok().map(|n| n.to_string()))
}

fn create_audio_output() -> Result<AudioOutput, String> {
    let (stream, handle) = OutputStream::try_default()
        .map_err(|e| format!("Failed to open audio output: {}", e))?;
    let sink = Sink::try_new(&handle)
        .map_err(|e| format!("Failed to create audio sink: {}", e))?;
    Ok(AudioOutput { _stream: stream, sink })
}

impl AudioSystem {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        let (tx, rx) = channel();

        thread::spawn(move || {
            let mut output: Option<AudioOutput> = None;
            let mut last_device_name: Option<String> = None;
            let mut last_device_check = Instant::now();
            let mut current_volume: f32 = 1.0;

            // Playback state for restoring after device switch
            let mut saved_path: Option<String> = None;
            let mut saved_position: Option<Duration> = None;
            let mut was_playing = false;

            // Manual position tracking (rodio's get_pos doesn't reflect seeks)
            let mut pos_base: Duration = Duration::ZERO;
            let mut pos_timer: Option<Instant> = None;

            // Initialize audio
            match create_audio_output() {
                Ok(o) => {
                    last_device_name = get_default_device_name();
                    println!("AudioSystem: Initialized with device: {:?}", last_device_name);
                    output = Some(o);
                }
                Err(e) => eprintln!("AudioSystem: {}", e),
            }

            loop {
                match rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(cmd) => match cmd {
                        AudioCommand::Play(path) => {
                            if let Some(ref o) = output {
                                if !o.sink.empty() {
                                    o.sink.stop();
                                }
                                match File::open(&path) {
                                    Ok(file) => match Decoder::new(BufReader::new(file)) {
                                        Ok(source) => {
                                            o.sink.append(source);
                                            o.sink.play();
                                            o.sink.set_volume(current_volume);
                                            saved_path = Some(path.clone());
                                            saved_position = None;
                                            was_playing = true;
                                            pos_base = Duration::ZERO;
                                            pos_timer = Some(Instant::now());
                                            println!("AudioSystem: Playing {}", path);
                                        }
                                        Err(e) => eprintln!("AudioSystem: Error decoding audio: {}", e),
                                    },
                                    Err(e) => eprintln!("AudioSystem: Error opening file: {}", e),
                                }
                            }
                        }
                        AudioCommand::Pause => {
                            if let Some(ref o) = output {
                                o.sink.pause();
                                if let Some(t) = pos_timer {
                                    pos_base += t.elapsed();
                                }
                                pos_timer = None;
                                was_playing = false;
                            }
                        }
                        AudioCommand::Resume => {
                            if let Some(ref o) = output {
                                o.sink.play();
                                was_playing = true;
                                pos_timer = Some(Instant::now());
                            }
                        }
                        AudioCommand::SetVolume(vol) => {
                            current_volume = vol;
                            if let Some(ref o) = output {
                                o.sink.set_volume(vol);
                            }
                        }
                        AudioCommand::Seek(seconds) => {
                            if let Some(ref o) = output {
                                let _ = o.sink.try_seek(Duration::from_secs_f64(seconds));
                                pos_base = Duration::from_secs_f64(seconds);
                                pos_timer = Some(Instant::now());
                            }
                        }
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Check for device changes every 2 seconds
                        if last_device_check.elapsed() >= Duration::from_secs(2) {
                            last_device_check = Instant::now();
                            let current_device = get_default_device_name();

                            if current_device.is_some() && current_device != last_device_name {
                                println!(
                                    "AudioSystem: Device changed {:?} -> {:?}, reconnecting...",
                                    last_device_name, current_device
                                );

                                // Save state before destroying
                                if let Some(ref o) = output {
                                    saved_position = Some(get_current_pos(pos_base, pos_timer));
                                    was_playing = !o.sink.is_paused() && !o.sink.empty();
                                }
                                output = None;

                                // Create new output
                                match create_audio_output() {
                                    Ok(new_output) => {
                                        new_output.sink.set_volume(current_volume);
                                        last_device_name = current_device;
                                        output = Some(new_output);
                                        println!("AudioSystem: Reconnected to device: {:?}", last_device_name);

                                        // Restore playback
                                        if let (Some(ref path), Some(pos)) = (&saved_path, saved_position) {
                                            match File::open(path) {
                                                Ok(file) => match Decoder::new(BufReader::new(file)) {
                                                    Ok(source) => {
                                                        if let Some(ref mut o) = output {
                                                            o.sink.append(source);
                                                            let _ = o.sink.try_seek(pos);
                                                            if was_playing {
                                                                o.sink.play();
                                                                pos_base = pos;
                                                                pos_timer = Some(Instant::now());
                                                            } else {
                                                                o.sink.pause();
                                                                pos_base = pos;
                                                                pos_timer = None;
                                                            }
                                                            o.sink.set_volume(current_volume);
                                                            println!("AudioSystem: Restored playback after device switch");
                                                        }
                                                    }
                                                    Err(e) => eprintln!("AudioSystem: Error decoding after switch: {}", e),
                                                },
                                                Err(e) => eprintln!("AudioSystem: Error opening after switch: {}", e),
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("AudioSystem: {}", e),
                                }
                                continue;
                            }
                        }

                        // Emit progress / detect track finished
                        if let Some(ref o) = output {
                            if was_playing {
                                if o.sink.empty() {
                                    was_playing = false;
                                    pos_timer = None;
                                    println!("AudioSystem: Track finished");
                                    let _ = app_handle.emit("track-finished", ());
                                } else {
                                    let pos = get_current_pos(pos_base, pos_timer);
                                    let _ = app_handle.emit("playback-progress", pos.as_secs_f64());
                                }
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        Self { command_tx: tx }
    }

    pub fn play(&self, path: &str) {
        let _ = self.command_tx.send(AudioCommand::Play(path.to_string()));
    }

    pub fn pause(&self) {
        let _ = self.command_tx.send(AudioCommand::Pause);
    }

    pub fn resume(&self) {
        let _ = self.command_tx.send(AudioCommand::Resume);
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(AudioCommand::SetVolume(volume));
    }

    pub fn seek(&self, seconds: f64) {
        let _ = self.command_tx.send(AudioCommand::Seek(seconds));
    }
}

fn get_current_pos(pos_base: Duration, pos_timer: Option<Instant>) -> Duration {
    match pos_timer {
        Some(t) => pos_base + t.elapsed(),
        None => pos_base,
    }
}
