use std::path::Path;



use super::assets::{load_installed_theme, resolve_playback_asset};

use super::{

    PlaybackOutcome, PlaybackRequest, SoundBackend, SoundBackendFactory,

    SoundError, SoundEvent, SoundRouting, SoundTheme, builtin_8_bit_theme,

};

use super::backend::default_backend_factory;



pub struct SoundEngine {

    backend: Box<dyn SoundBackend>,

}



impl SoundEngine {

    pub fn try_new(factory: &dyn SoundBackendFactory) -> Result<Self, SoundError> {

        Ok(Self {

            backend: factory.create()?,

        })

    }



    pub fn with_backend(backend: Box<dyn SoundBackend>) -> Self {

        Self { backend }

    }



    pub fn with_default_backend() -> Result<Self, SoundError> {

        Self::try_new(default_backend_factory().as_ref())

    }



    pub fn backend_id(&self) -> &str {

        self.backend.backend_id()

    }



    pub fn play_event(

        &self,

        themes_root: &Path,

        theme_id: &str,

        event: SoundEvent,

        routing: &SoundRouting,

        agent: Option<&str>,

        local_minute: u16,

    ) -> Result<PlaybackOutcome, SoundError> {

        let theme = resolve_theme(themes_root, theme_id)?;

        let asset = theme

            .events

            .get(&event)

            .ok_or_else(|| SoundError::Backend(format!("theme {theme_id} has no asset for {event:?}")))?;

        let volume = routing.effective_volume(event, agent, local_minute)?;

        let reason = playback_skip_reason(routing, volume);

        let Some(volume) = volume.filter(|volume| *volume > 0.0) else {

            return Ok(PlaybackOutcome {

                played: false,

                effective_volume: volume,

                reason,

            });

        };



        let audio = resolve_playback_asset(themes_root, &theme, asset)?;

        let request = PlaybackRequest {

            event,

            agent: agent.map(str::to_owned),

            audio,

            volume,

        };

        self.backend.play(&request)?;

        Ok(PlaybackOutcome {

            played: true,

            effective_volume: Some(volume),

            reason: None,

        })

    }



    pub fn stop_all(&self) -> Result<(), SoundError> {

        self.backend.stop_all()

    }

}



fn resolve_theme(themes_root: &Path, theme_id: &str) -> Result<SoundTheme, SoundError> {

    if theme_id == "builtin.8-bit" {

        let theme = builtin_8_bit_theme();

        theme.validate()?;

        return Ok(theme);

    }

    load_installed_theme(themes_root, theme_id)

}



fn playback_skip_reason(routing: &SoundRouting, volume: Option<f32>) -> Option<String> {

    if volume.is_some() {

        return None;

    }

    if !routing.enabled {

        Some("sound is disabled".into())

    } else {

        Some("quiet hours are active".into())

    }

}



#[cfg(test)]

mod tests {

    use std::collections::BTreeMap;

    use std::sync::{Arc, Mutex};



    use super::*;

    use crate::sound::{QuietHours, ResolvedAudio};

    use crate::sound_pack::fixtures::{build_zip, pack_entries, sample_theme};

    use crate::sound_pack::{reserved_theme_ids, validate_and_install_pack_bytes};



    struct RecordingState {

        requests: Vec<(SoundEvent, Option<String>, ResolvedAudio, f32)>,

    }



    impl Default for RecordingState {

        fn default() -> Self {

            Self {

                requests: Vec::new(),

            }

        }

    }



    struct RecordingBackend {

        state: Arc<Mutex<RecordingState>>,

    }



    impl RecordingBackend {

        fn shared() -> (Arc<Mutex<RecordingState>>, Self) {

            let state = Arc::new(Mutex::new(RecordingState::default()));

            (

                Arc::clone(&state),

                Self { state },

            )

        }

    }



    impl SoundBackend for RecordingBackend {

        fn backend_id(&self) -> &str {

            "recording"

        }



        fn play(&self, request: &PlaybackRequest) -> Result<(), SoundError> {

            let mut state = self.state.lock().unwrap();

            state.requests.push((

                request.event,

                request.agent.clone(),

                request.audio.clone(),

                request.volume,

            ));

            Ok(())

        }



        fn stop_all(&self) -> Result<(), SoundError> {

            Ok(())

        }

    }



    fn routing() -> SoundRouting {

        SoundRouting {

            enabled: true,

            volume: 0.8,

            quiet_hours: None,

            event_volume: BTreeMap::from([(SoundEvent::Completed, 0.5)]),

            agent_volume: BTreeMap::from([("codex".into(), 0.5)]),

        }

    }



    fn temp_themes_root() -> tempfile::TempDir {

        tempfile::TempDir::new().expect("tempdir")

    }



    #[test]

    fn play_event_honors_combined_volume_and_asset_path() {

        let themes_root = temp_themes_root();

        let (state, backend) = RecordingBackend::shared();

        let engine = SoundEngine::with_backend(Box::new(backend));

        let outcome = engine

            .play_event(

                themes_root.path(),

                "builtin.8-bit",

                SoundEvent::Completed,

                &routing(),

                Some("codex"),

                720,

            )

            .unwrap();

        assert!(outcome.played);

        assert_eq!(outcome.effective_volume, Some(0.2));

        let requests = state.lock().unwrap().requests.clone();

        assert_eq!(requests.len(), 1);

        assert_eq!(requests[0].0, SoundEvent::Completed);

        assert_eq!(requests[0].1.as_deref(), Some("codex"));

        assert!(matches!(requests[0].2, ResolvedAudio::Embedded(_)));

        assert!((requests[0].3 - 0.2).abs() < f32::EPSILON);

    }



    #[test]

    fn play_event_resolves_installed_theme_assets() {

        let themes_root = temp_themes_root();

        let theme = sample_theme();

        let entries = pack_entries(&theme, b"RIFF");

        let zip = build_zip(&entries);

        validate_and_install_pack_bytes(&zip, themes_root.path(), &reserved_theme_ids())

            .expect("install");



        let (state, backend) = RecordingBackend::shared();

        let engine = SoundEngine::with_backend(Box::new(backend));

        let outcome = engine

            .play_event(

                themes_root.path(),

                "community.test-pack",

                SoundEvent::Completed,

                &routing(),

                None,

                720,

            )

            .unwrap();

        assert!(outcome.played);

        let requests = state.lock().unwrap().requests.clone();

        assert_eq!(requests.len(), 1);

        assert!(matches!(requests[0].2, ResolvedAudio::File(_)));

    }



    #[test]

    fn play_event_skips_during_quiet_hours_without_touching_backend() {

        let themes_root = temp_themes_root();

        let (state, backend) = RecordingBackend::shared();

        let engine = SoundEngine::with_backend(Box::new(backend));

        let mut routing = routing();

        routing.quiet_hours = Some(QuietHours {

            start_minute: 600,

            end_minute: 900,

        });

        let outcome = engine

            .play_event(

                themes_root.path(),

                "builtin.8-bit",

                SoundEvent::Completed,

                &routing,

                Some("codex"),

                720,

            )

            .unwrap();

        assert!(!outcome.played);

        assert_eq!(outcome.effective_volume, None);

        assert_eq!(outcome.reason.as_deref(), Some("quiet hours are active"));

        assert!(state.lock().unwrap().requests.is_empty());

    }



    #[test]

    fn play_event_skips_when_sound_is_disabled() {

        let themes_root = temp_themes_root();

        let (state, backend) = RecordingBackend::shared();

        let engine = SoundEngine::with_backend(Box::new(backend));

        let mut routing = routing();

        routing.enabled = false;

        let outcome = engine

            .play_event(

                themes_root.path(),

                "builtin.8-bit",

                SoundEvent::Notification,

                &routing,

                None,

                720,

            )

            .unwrap();

        assert!(!outcome.played);

        assert_eq!(outcome.reason.as_deref(), Some("sound is disabled"));

        assert!(state.lock().unwrap().requests.is_empty());

    }



    #[test]

    fn stub_backend_reports_unsupported_platform() {

        let backend = crate::sound::backend::StubBackendFactory

            .create()

            .expect("stub factory");

        assert_eq!(backend.backend_id(), "stub");

        let request = PlaybackRequest {

            event: SoundEvent::Notification,

            agent: None,

            audio: ResolvedAudio::Embedded(

                super::super::assets::resolve_builtin_asset("builtin/8-bit/notification.wav")

                    .unwrap(),

            ),

            volume: 0.5,

        };

        assert!(matches!(

            backend.play(&request),

            Err(SoundError::UnsupportedPlatform)

        ));

    }

}


