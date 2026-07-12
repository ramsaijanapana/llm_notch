use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};

use rodio::mixer::Mixer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use super::super::{PlaybackRequest, ResolvedAudio, SoundBackend, SoundBackendFactory, SoundError};

#[cfg(not(target_os = "macos"))]
struct RodioBackend {
    stream: OutputStream,
    sinks: Arc<Mutex<Vec<Sink>>>,
}

#[cfg(target_os = "macos")]
struct RodioBackend {
    command_tx: std::sync::mpsc::SyncSender<WorkerCommand>,
    _worker: std::thread::JoinHandle<()>,
}

#[cfg(target_os = "macos")]
enum WorkerCommand {
    Play(PlaybackRequest, std::sync::mpsc::SyncSender<Result<(), SoundError>>),
    StopAll(std::sync::mpsc::SyncSender<Result<(), SoundError>>),
}

#[cfg(target_os = "macos")]
fn play_on_mixer(
    mixer: &Mixer,
    sinks: &mut Vec<Sink>,
    request: &PlaybackRequest,
) -> Result<(), SoundError> {
    match &request.audio {
        ResolvedAudio::Embedded(bytes) => {
            let cursor = Cursor::new(bytes.to_vec());
            let source = Decoder::new(cursor)
                .map_err(|error| SoundError::Backend(format!("failed to decode audio: {error}")))?
                .amplify(request.volume);
            let sink = Sink::connect_new(mixer);
            sink.append(source);
            sinks.retain(|existing| !existing.empty());
            sinks.push(sink);
        }
        ResolvedAudio::File(path) => {
            let file = File::open(path).map_err(|error| {
                SoundError::Backend(format!(
                    "failed to open sound asset {}: {error}",
                    path.display()
                ))
            })?;
            let source = Decoder::new(BufReader::new(file))
                .map_err(|error| SoundError::Backend(format!("failed to decode audio: {error}")))?
                .amplify(request.volume);
            let sink = Sink::connect_new(mixer);
            sink.append(source);
            sinks.retain(|existing| !existing.empty());
            sinks.push(sink);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
impl RodioBackend {
    fn register_sink(&self, sink: Sink) -> Result<(), SoundError> {
        let mut sinks = self
            .sinks
            .lock()
            .map_err(|_| SoundError::Backend("audio sink registry poisoned".into()))?;
        sinks.retain(|existing| !existing.empty());
        sinks.push(sink);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl RodioBackend {
    fn spawn_worker(stream: OutputStream) -> Result<Self, SoundError> {
        let (command_tx, command_rx) = std::sync::mpsc::sync_channel(8);
        let mixer = stream.mixer().clone();
        let worker = std::thread::spawn(move || {
            let mut sinks = Vec::<Sink>::new();
            let _stream = stream;
            while let Ok(command) = command_rx.recv() {
                match command {
                    WorkerCommand::Play(request, reply) => {
                        let result = play_on_mixer(&mixer, &mut sinks, &request);
                        let _ = reply.send(result);
                    }
                    WorkerCommand::StopAll(reply) => {
                        for sink in sinks.drain(..) {
                            sink.stop();
                        }
                        let _ = reply.send(Ok(()));
                    }
                }
            }
        });
        Ok(Self {
            command_tx,
            _worker: worker,
        })
    }

    fn dispatch(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<(), SoundError>>) -> WorkerCommand,
    ) -> Result<(), SoundError> {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(0);
        self.command_tx
            .send(build(reply_tx))
            .map_err(|_| SoundError::Backend("audio worker unavailable".into()))?;
        reply_rx
            .recv()
            .map_err(|_| SoundError::Backend("audio worker dropped reply".into()))?
    }
}

impl SoundBackend for RodioBackend {
    fn backend_id(&self) -> &str {
        #[cfg(windows)]
        {
            "windows-wasapi"
        }
        #[cfg(target_os = "macos")]
        {
            "macos-coreaudio"
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            "rodio"
        }
    }

    fn play(&self, request: &PlaybackRequest) -> Result<(), SoundError> {
        #[cfg(target_os = "macos")]
        {
            let request = request.clone();
            return self.dispatch(|reply| WorkerCommand::Play(request, reply));
        }

        #[cfg(not(target_os = "macos"))]
        {
            match &request.audio {
                ResolvedAudio::Embedded(bytes) => {
                    let cursor = Cursor::new(bytes.to_vec());
                    let source = Decoder::new(cursor)
                        .map_err(|error| {
                            SoundError::Backend(format!("failed to decode audio: {error}"))
                        })?
                        .amplify(request.volume);
                    let sink = Sink::connect_new(self.stream.mixer());
                    sink.append(source);
                    self.register_sink(sink)?;
                }
                ResolvedAudio::File(path) => {
                    let file = File::open(path).map_err(|error| {
                        SoundError::Backend(format!(
                            "failed to open sound asset {}: {error}",
                            path.display()
                        ))
                    })?;
                    let source = Decoder::new(BufReader::new(file))
                        .map_err(|error| {
                            SoundError::Backend(format!("failed to decode audio: {error}"))
                        })?
                        .amplify(request.volume);
                    let sink = Sink::connect_new(self.stream.mixer());
                    sink.append(source);
                    self.register_sink(sink)?;
                }
            }
            Ok(())
        }
    }

    fn stop_all(&self) -> Result<(), SoundError> {
        #[cfg(target_os = "macos")]
        {
            return self.dispatch(|reply| WorkerCommand::StopAll(reply));
        }

        #[cfg(not(target_os = "macos"))]
        {
            let mut sinks = self
                .sinks
                .lock()
                .map_err(|_| SoundError::Backend("audio sink registry poisoned".into()))?;
            for sink in sinks.drain(..) {
                sink.stop();
            }
            Ok(())
        }
    }
}

pub struct RodioBackendFactory;

impl SoundBackendFactory for RodioBackendFactory {
    fn create(&self) -> Result<Box<dyn SoundBackend>, SoundError> {
        let stream = OutputStreamBuilder::open_default_stream().map_err(|error| {
            #[cfg(windows)]
            let message = format!("failed to open WASAPI output stream: {error}");
            #[cfg(target_os = "macos")]
            let message = format!("failed to open CoreAudio output stream: {error}");
            #[cfg(not(any(windows, target_os = "macos")))]
            let message = format!("failed to open audio output stream: {error}");
            SoundError::Backend(message)
        })?;

        #[cfg(target_os = "macos")]
        {
            return Ok(Box::new(RodioBackend::spawn_worker(stream)?));
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(Box::new(RodioBackend {
                stream,
                sinks: Arc::new(Mutex::new(Vec::new())),
            }))
        }
    }
}

