use super::super::{PlaybackRequest, SoundBackend, SoundBackendFactory, SoundError};

pub struct StubBackend;

impl SoundBackend for StubBackend {
    fn backend_id(&self) -> &str {
        "stub"
    }

    fn play(&self, _request: &PlaybackRequest) -> Result<(), SoundError> {
        Err(SoundError::UnsupportedPlatform)
    }

    fn stop_all(&self) -> Result<(), SoundError> {
        Err(SoundError::UnsupportedPlatform)
    }
}

pub struct StubBackendFactory;

impl SoundBackendFactory for StubBackendFactory {
    fn create(&self) -> Result<Box<dyn SoundBackend>, SoundError> {
        Ok(Box::new(StubBackend))
    }
}
