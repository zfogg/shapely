//! Procedural audio cues used by combat, pickups, and room transitions.

use bevy::prelude::*;
use std::f32::consts::TAU;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum SoundKind {
    Hit,
    Attack,
    Pickup,
    Transition,
}

#[derive(Resource, Clone)]
pub(crate) struct AudioCues {
    pub(crate) hit: Handle<AudioSource>,
    pub(crate) attack: Handle<AudioSource>,
    pub(crate) pickup: Handle<AudioSource>,
    pub(crate) transition: Handle<AudioSource>,
}

pub(crate) fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let add_tone =
        |frequency: f32, duration: f32, volume: f32, sources: &mut Assets<AudioSource>| {
            sources.add(AudioSource {
                bytes: Arc::from(tone_wav(frequency, duration, volume)),
            })
        };
    commands.insert_resource(AudioCues {
        hit: add_tone(180.0, 0.08, 0.22, &mut sources),
        attack: add_tone(620.0, 0.10, 0.14, &mut sources),
        pickup: add_tone(880.0, 0.16, 0.16, &mut sources),
        transition: add_tone(110.0, 0.30, 0.12, &mut sources),
    });
}

fn tone_wav(frequency: f32, duration: f32, volume: f32) -> Vec<u8> {
    let sample_rate = 22_050_u32;
    let samples = (duration * sample_rate as f32) as u32;
    let data_size = samples * 2;
    let mut bytes = Vec::with_capacity((44 + data_size) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / sample_rate as f32;
        let envelope = 1.0 - (time / duration).min(1.0);
        let sample = (time * frequency * TAU).sin() * envelope * volume * i16::MAX as f32;
        bytes.extend_from_slice(&(sample as i16).to_le_bytes());
    }
    bytes
}

pub(crate) fn play_sound(commands: &mut Commands, cues: Option<&AudioCues>, sound: SoundKind) {
    let Some(cues) = cues else { return };
    let source = match sound {
        SoundKind::Hit => &cues.hit,
        SoundKind::Attack => &cues.attack,
        SoundKind::Pickup => &cues.pickup,
        SoundKind::Transition => &cues.transition,
    };
    commands.spawn(AudioBundle {
        source: source.clone(),
        settings: PlaybackSettings::DESPAWN,
    });
}
