use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use super::super::{PlaybackRequest, ResolvedAudio, SoundBackend, SoundBackendFactory, SoundError};

struct RodioBackend {
    stream: OutputStream,
    sinks: Arc<Mutex<Vec<Sink>>>,
}

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

    fn stop_all(&self) -> Result<(), SoundError> {
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
        Ok(Box::new(RodioBackend {
            stream,
            sinks: Arc::new(Mutex::new(Vec::new())),
        }))
    }
}
