#[cfg(any(windows, target_os = "macos"))]
mod rodio;
mod stub;

#[cfg(any(windows, target_os = "macos"))]
pub use rodio::RodioBackendFactory;
pub use stub::{StubBackend, StubBackendFactory};

pub fn default_backend_factory() -> Box<dyn super::SoundBackendFactory> {
    #[cfg(any(windows, target_os = "macos"))]
    {
        Box::new(RodioBackendFactory)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Box::new(StubBackendFactory)
    }
}

pub fn native_playback_supported() -> bool {
    #[cfg(any(windows, target_os = "macos"))]
    {
        true
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}
