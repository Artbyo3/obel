use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use tauri::Emitter;

pub enum AudioCommand {
    Play(String),
    Pause,
    Resume,
    #[allow(dead_code)]
    Stop,
    SetVolume(f32),
}

pub struct AudioSystem {
    command_tx: Sender<AudioCommand>,
}

impl AudioSystem {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        let (tx, rx) = channel();

        thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();
            let mut playing_started = false;

            loop {
                // Check for commands with a timeout to allow monitoring the sink
                match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(cmd) => match cmd {
                        AudioCommand::Play(path) => {
                            if !sink.empty() {
                                sink.stop();
                            }
                            match File::open(&path) {
                                Ok(file) => match Decoder::new(BufReader::new(file)) {
                                    Ok(source) => {
                                        sink.append(source);
                                        sink.play();
                                        playing_started = true;
                                        println!("AudioSystem: Playing {}", path);
                                    }
                                    Err(e) => eprintln!("Error decoding audio: {}", e),
                                },
                                Err(e) => eprintln!("Error opening file: {}", e),
                            }
                        }
                        AudioCommand::Pause => sink.pause(),
                        AudioCommand::Resume => sink.play(),
                        AudioCommand::Stop => {
                            sink.stop();
                            playing_started = false;
                        }
                        AudioCommand::SetVolume(vol) => sink.set_volume(vol),
                    },
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Monitor sink status when we don't have new commands
                        if playing_started && sink.empty() {
                            playing_started = false;
                            println!("AudioSystem: Track finished, emitting event");
                            let _ = app_handle.emit("track-finished", ());
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

    #[allow(dead_code)]
    pub fn stop(&self) {
        let _ = self.command_tx.send(AudioCommand::Stop);
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(AudioCommand::SetVolume(volume));
    }
}
