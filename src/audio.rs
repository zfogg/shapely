//! Procedural audio cues used by combat, pickups, and room transitions.

use bevy::prelude::*;
use std::f32::consts::TAU;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum SoundKind {
    Hit,
    PlayerLaser,
    PlayerSlash,
    PlayerFireball,
    EnemyMinionAttack,
    EnemyThugAttack,
    EnemyMinibossAttack,
    Pickup,
    ItemUse,
    ChestOpen,
    UiSelect,
    Transition,
}

#[derive(Resource, Clone)]
pub(crate) struct AudioCues {
    pub(crate) hit: Handle<AudioSource>,
    pub(crate) player_laser: Handle<AudioSource>,
    pub(crate) player_slash: Handle<AudioSource>,
    pub(crate) player_fireball: Handle<AudioSource>,
    pub(crate) enemy_minion_attack: Handle<AudioSource>,
    pub(crate) enemy_thug_attack: Handle<AudioSource>,
    pub(crate) enemy_miniboss_attack: Handle<AudioSource>,
    pub(crate) pickup: Handle<AudioSource>,
    pub(crate) item_use: Handle<AudioSource>,
    pub(crate) chest_open: Handle<AudioSource>,
    pub(crate) ui_select: Handle<AudioSource>,
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
        player_laser: add_tone(1_240.0, 0.09, 0.13, &mut sources),
        player_slash: add_tone(360.0, 0.12, 0.16, &mut sources),
        player_fireball: add_tone(220.0, 0.20, 0.16, &mut sources),
        enemy_minion_attack: add_tone(145.0, 0.14, 0.15, &mut sources),
        enemy_thug_attack: add_tone(510.0, 0.16, 0.14, &mut sources),
        enemy_miniboss_attack: add_tone(82.0, 0.32, 0.18, &mut sources),
        pickup: add_tone(880.0, 0.16, 0.16, &mut sources),
        item_use: add_tone(740.0, 0.18, 0.15, &mut sources),
        chest_open: add_tone(300.0, 0.55, 0.18, &mut sources),
        ui_select: add_tone(1_050.0, 0.055, 0.11, &mut sources),
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
        SoundKind::PlayerLaser => &cues.player_laser,
        SoundKind::PlayerSlash => &cues.player_slash,
        SoundKind::PlayerFireball => &cues.player_fireball,
        SoundKind::EnemyMinionAttack => &cues.enemy_minion_attack,
        SoundKind::EnemyThugAttack => &cues.enemy_thug_attack,
        SoundKind::EnemyMinibossAttack => &cues.enemy_miniboss_attack,
        SoundKind::Pickup => &cues.pickup,
        SoundKind::ItemUse => &cues.item_use,
        SoundKind::ChestOpen => &cues.chest_open,
        SoundKind::UiSelect => &cues.ui_select,
        SoundKind::Transition => &cues.transition,
    };
    commands.spawn(AudioBundle {
        source: source.clone(),
        settings: PlaybackSettings::DESPAWN,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_cues_have_distinct_waveforms() {
        let laser = tone_wav(1_240.0, 0.09, 0.13);
        let slash = tone_wav(360.0, 0.12, 0.16);
        let fireball = tone_wav(220.0, 0.20, 0.16);
        assert_ne!(laser, slash);
        assert_ne!(slash, fireball);
        assert_ne!(laser, fireball);
    }

    #[test]
    fn generated_cues_are_valid_wav_files() {
        let wav = tone_wav(440.0, 0.1, 0.2);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
    }
}
