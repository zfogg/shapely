//! Player state, movement, abilities, and player-facing combat helpers.
//!
//! The three abilities are represented by the same [`Attack`] component and
//! differ through [`AttackKind`] and [`AttackProfile`].

use crate::damageable::Damageable;
use crate::projectable::{FxKind, FxMaterial};
use crate::{audio, items, ui};
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;

#[derive(Component)]
/// The controllable square character and its damage state.
pub struct Player {
    pub hit_flash: f32,
    pub health: i32,
    pub max_health: i32,
}

impl Damageable for Player {
    fn health(&self) -> i32 {
        self.health
    }
    fn max_health(&self) -> i32 {
        self.max_health
    }
    fn take_damage(&mut self, amount: i32) {
        self.health = (self.health - amount).max(0);
    }
}

#[derive(Component)]
/// A temporary velocity used for knockback impulses.
pub struct BounceVelocity(pub Vec2);

#[derive(Component)]
/// A player attack moving through the room or animating in place.
pub struct Attack {
    pub kind: AttackKind,
    pub damage: i32,
    pub velocity: Vec2,
    pub age: f32,
    pub hit_targets: Vec<Entity>,
    pub animation_speed: f32,
    pub rotation_offset: f32,
}

#[derive(Clone, Copy, PartialEq)]
/// The player's three ability types.
pub enum AttackKind {
    Laser,
    Slash,
    Fireball,
}

/// Tunable combat values for one ability.
pub struct AttackProfile {
    pub damage: i32,
    pub speed: f32,
    pub max_age: f32,
    pub size: Vec2,
    pub mana_cost: i32,
    pub cooldown: f32,
}

/// Returns the design profile for an ability.
pub fn attack_profile(kind: AttackKind) -> AttackProfile {
    match kind {
        AttackKind::Laser => AttackProfile {
            damage: 5,
            speed: 900.0,
            max_age: 1.2,
            size: Vec2::new(130.0, 8.0),
            mana_cost: 8,
            cooldown: 0.45,
        },
        AttackKind::Slash => AttackProfile {
            damage: 8,
            speed: 0.0,
            max_age: 0.22,
            size: Vec2::splat(120.0),
            mana_cost: 0,
            cooldown: 0.45,
        },
        AttackKind::Fireball => AttackProfile {
            damage: 7,
            speed: 330.0,
            max_age: 1.5,
            size: Vec2::splat(34.0),
            mana_cost: 8,
            cooldown: 0.45,
        },
    }
}

/// Returns the current effective interval for an attack.
pub fn effective_attack_cooldown(kind: AttackKind, state: &crate::GameState) -> f32 {
    let index = match kind {
        AttackKind::Laser => 0,
        AttackKind::Slash => 1,
        AttackKind::Fireball => 2,
    };
    attack_profile(kind).cooldown
        * state.cooldown_multiplier
        * state.attack_cooldown_multipliers[index]
        * if state.attack_speed_boost > 0.0 {
            0.75
        } else {
            1.0
        }
}

/// Tests whether an attack overlaps any part of an enemy shape.
pub fn attack_hits_enemy(
    attack_transform: &Transform,
    kind: AttackKind,
    enemy_position: Vec2,
    enemy_radius: f32,
) -> bool {
    let relative = enemy_position - attack_transform.translation.truncate();
    match kind {
        AttackKind::Slash => relative.length() <= 60.0 + enemy_radius,
        AttackKind::Fireball => relative.length() <= 18.0 + enemy_radius,
        AttackKind::Laser => {
            let angle = attack_transform.rotation.to_euler(EulerRot::ZYX).0;
            let local = Vec2::new(
                relative.x * angle.cos() + relative.y * angle.sin(),
                -relative.x * angle.sin() + relative.y * angle.cos(),
            );
            local.x.abs() <= 65.0 + enemy_radius && local.y.abs() <= 4.0 + enemy_radius
        }
    }
}

/// Moves the player while applying active knockback and speed boosts.
pub fn player_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut query: Query<(&mut Transform, &mut BounceVelocity), With<Player>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if let Ok((mut transform, mut bounce)) = query.get_single_mut() {
        if bounce.0.length_squared() > 1.0 {
            transform.translation += bounce.0.extend(0.0) * time.delta_seconds();
            bounce.0 = bounce
                .0
                .lerp(Vec2::ZERO, (10.0 * time.delta_seconds()).min(1.0));
            return;
        }
        let speed = crate::PLAYER_SPEED
            * state.move_speed_multiplier
            * if state.speed_boost > 0.0 { 1.55 } else { 1.0 };
        transform.translation +=
            direction.normalize_or_zero().extend(0.0) * speed * time.delta_seconds();
        transform.translation.x = transform.translation.x.clamp(-570.0, 570.0);
        transform.translation.y = transform.translation.y.clamp(-320.0, 320.0);
    }
}

/// Maps the G502 button events exposed by winit to player abilities.
///
/// Windows exposes G4/G5 as the standard Back/Forward buttons. G7 is a
/// Right-click is also accepted for slash, which avoids depending on G HUB's
/// handling of the G7 DPI button.
pub fn mouse_attack_kind(mouse: &ButtonInput<MouseButton>) -> Option<AttackKind> {
    if mouse.just_pressed(MouseButton::Forward) {
        Some(AttackKind::Laser)
    } else if mouse.just_pressed(MouseButton::Back) {
        Some(AttackKind::Fireball)
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(AttackKind::Slash)
    } else {
        None
    }
}

/// Reads keyboard and G502 mouse ability input and aims attacks at the mouse cursor.
pub fn attack_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut state: ResMut<crate::GameState>,
    player: Query<&Transform, With<Player>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FxMaterial>>,
    cues: Option<Res<audio::AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(player) = player.get_single() else {
        return;
    };
    let kind = if keys.just_pressed(KeyCode::Digit1) {
        Some(AttackKind::Laser)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(AttackKind::Slash)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(AttackKind::Fireball)
    } else {
        mouse_attack_kind(&mouse)
    };
    let Some(kind) = kind else { return };
    let index = match kind {
        AttackKind::Laser => 0,
        AttackKind::Slash => 1,
        AttackKind::Fireball => 2,
    };
    // Keep an individual timer for future per-attack upgrades, but enforce a
    // shared timer so changing abilities cannot bypass the attack rate.
    if state.cooldowns[index] > 0.0 || state.attack_timer > 0.0 {
        return;
    }
    let profile = attack_profile(kind);
    if state.mana < profile.mana_cost {
        return;
    }
    state.mana -= profile.mana_cost;
    let (camera, camera_transform) = cameras.single();
    let Some(cursor) = windows
        .single()
        .cursor_position()
        .and_then(|position| camera.viewport_to_world_2d(camera_transform, position))
    else {
        return;
    };
    let direction = (cursor - player.translation.truncate()).normalize_or_zero();
    if direction == Vec2::ZERO {
        return;
    }
    let cooldown = effective_attack_cooldown(kind, &state);
    state.cooldowns[index] = cooldown;
    state.attack_timer = cooldown;
    let spawn_position =
        player.translation + direction.extend(0.0) * 42.0 + Vec3::new(0.0, 0.0, 4.0);
    let fx_kind = match kind {
        AttackKind::Laser => FxKind::Laser,
        AttackKind::Slash => FxKind::Slash,
        AttackKind::Fireball => FxKind::Fireball,
    };
    let damage = ((profile.damage as f32 * state.power_multiplier).round() as i32).max(1);
    let rotation = direction.y.atan2(direction.x);
    let spawn = |commands: &mut Commands,
                 meshes: &mut Assets<Mesh>,
                 materials: &mut Assets<FxMaterial>,
                 position: Vec3,
                 lifetime: f32,
                 animation_speed: f32,
                 rotation_offset: f32| {
        let mesh = if kind == AttackKind::Fireball {
            meshes.add(Circle::new(profile.size.x * 0.5)).into()
        } else {
            meshes
                .add(Rectangle::new(profile.size.x, profile.size.y))
                .into()
        };
        commands.spawn((
            Attack {
                kind,
                damage,
                velocity: direction * profile.speed,
                age: 0.0,
                hit_targets: Vec::new(),
                animation_speed,
                rotation_offset,
            },
            crate::Lifetime(lifetime),
            MaterialMesh2dBundle {
                mesh,
                material: materials.add(FxMaterial::new(fx_kind, 0.0, 1.0)),
                transform: Transform {
                    translation: position,
                    rotation: Quat::from_rotation_z(rotation + rotation_offset),
                    scale: Vec3::new(state.projectile_scale, state.projectile_scale, 1.0),
                },
                ..default()
            },
        ));
    };
    spawn(
        &mut commands,
        &mut meshes,
        &mut materials,
        spawn_position,
        profile.max_age,
        1.0,
        0.0,
    );
    if state.extra_attack {
        let side = Vec2::new(-direction.y, direction.x) * 12.0;
        let extra_position = spawn_position + side.extend(0.0);
        let animation_speed = if kind == AttackKind::Slash { 1.4 } else { 1.0 };
        let rotation_offset = if kind == AttackKind::Slash {
            std::f32::consts::PI
        } else {
            0.0
        };
        spawn(
            &mut commands,
            &mut meshes,
            &mut materials,
            extra_position,
            if kind == AttackKind::Slash {
                profile.max_age / 1.4
            } else {
                profile.max_age
            },
            animation_speed,
            rotation_offset,
        );
    }
    let sound = match kind {
        AttackKind::Laser => audio::SoundKind::PlayerLaser,
        AttackKind::Slash => audio::SoundKind::PlayerSlash,
        AttackKind::Fireball => audio::SoundKind::PlayerFireball,
    };
    audio::play_sound(&mut commands, cues.as_deref(), sound);
    let _ = time;
}

/// Advances the player's damage flash and restores its normal color.
pub fn player_flash(
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut player: Query<(&mut Player, &mut Sprite)>,
) {
    if state.mode == crate::GameMode::Title {
        return;
    }
    let Ok((mut player, mut sprite)) = player.get_single_mut() else {
        return;
    };
    player.hit_flash = (player.hit_flash - time.delta_seconds()).max(0.0);
    sprite.color = if player.hit_flash > 0.0 {
        Color::BLACK
    } else {
        Color::srgb(1.0, 0.12, 0.4)
    };
}

/// Collects nearby pickups into the player's 24-slot inventory.
pub(crate) fn collect_pickups(
    mut commands: Commands,
    mut state: ResMut<crate::GameState>,
    player: Query<&Transform, (With<Player>, Without<items::Pickup>)>,
    pickups: Query<(Entity, &items::Pickup, &Transform), Without<Player>>,
    assets: Option<Res<AssetServer>>,
    cues: Option<Res<audio::AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(player) = player.get_single() else {
        return;
    };
    for (entity, pickup, transform) in &pickups {
        if player.translation.distance(transform.translation) < 38.0
            && state.inventory.len() < items::potion::INVENTORY_CAPACITY
        {
            state.inventory.push(pickup.item);
            if let Some(assets) = assets.as_ref() {
                commands.spawn((
                    ui::FloatingText {
                        velocity: Vec2::new(0.0, 26.0),
                    },
                    crate::Lifetime(1.2),
                    Text2dBundle {
                        text: Text::from_section(
                            items::pickup_message(pickup.item),
                            TextStyle {
                                font: assets.load("fonts/FiraSans-Bold.ttf"),
                                font_size: 18.0,
                                color: items::item_color(pickup.item),
                            },
                        ),
                        transform: Transform::from_translation(
                            transform.translation + Vec3::new(0.0, 20.0, 8.0),
                        ),
                        ..default()
                    },
                ));
            }
            audio::play_sound(&mut commands, cues.as_deref(), audio::SoundKind::Pickup);
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Uses the keyboard inventory shortcuts.
pub(crate) fn use_inventory_items(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<crate::GameState>,
    cues: Option<Res<audio::AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let requested = if keys.just_pressed(KeyCode::KeyH) {
        Some(items::ItemKind::HealthPotion)
    } else if keys.just_pressed(KeyCode::KeyM) {
        Some(items::ItemKind::ManaPotion)
    } else if keys.just_pressed(KeyCode::KeyB) {
        Some(items::ItemKind::SpeedPotion)
    } else {
        None
    };
    if let Some(item) = requested {
        if consume_item(&mut state, item) {
            audio::play_sound(&mut commands, cues.as_deref(), audio::SoundKind::ItemUse);
        }
    }
}

/// Removes one matching item and applies its effect.
pub(crate) fn consume_item(state: &mut crate::GameState, item: items::ItemKind) -> bool {
    let Some(index) = state
        .inventory
        .iter()
        .position(|candidate| *candidate == item)
    else {
        return false;
    };
    state.inventory.remove(index);
    match item {
        items::ItemKind::HealthPotion => {
            state.health = (state.health + items::healthpotion::HEAL_AMOUNT).min(state.max_health)
        }
        items::ItemKind::ManaPotion => {
            state.mana = (state.mana + items::manapotion::RESTORE_AMOUNT).min(state.max_mana)
        }
        items::ItemKind::SpeedPotion => state.speed_boost = items::speedpotion::DURATION,
        items::ItemKind::AttackSpeedPotion => {
            state.attack_speed_boost = items::attackspeedpotion::DURATION
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemKind, Pickup};

    #[test]
    fn attack_profiles_match_damage_and_resource_design() {
        let laser = attack_profile(AttackKind::Laser);
        assert_eq!((laser.damage, laser.speed, laser.mana_cost), (5, 900.0, 8));
        let slash = attack_profile(AttackKind::Slash);
        assert_eq!((slash.damage, slash.speed, slash.mana_cost), (8, 0.0, 0));
        let fireball = attack_profile(AttackKind::Fireball);
        assert_eq!(
            (fireball.damage, fireball.speed, fireball.mana_cost),
            (7, 330.0, 8)
        );
    }

    #[test]
    fn attack_rate_is_shared_but_supports_future_per_attack_speed() {
        let mut state = crate::GameState::default();
        assert_eq!(effective_attack_cooldown(AttackKind::Laser, &state), 0.45);
        assert_eq!(effective_attack_cooldown(AttackKind::Slash, &state), 0.45);
        assert_eq!(
            effective_attack_cooldown(AttackKind::Fireball, &state),
            0.45
        );
        state.attack_cooldown_multipliers[2] = 0.8;
        assert_eq!(
            effective_attack_cooldown(AttackKind::Fireball, &state),
            0.45 * 0.8
        );
        state.attack_speed_boost = 1.0;
        assert_eq!(
            effective_attack_cooldown(AttackKind::Fireball, &state),
            0.45 * 0.8 * 0.75
        );
    }

    #[test]
    fn hitboxes_use_enemy_edges_and_attack_direction() {
        let transform = Transform::from_translation(Vec3::ZERO);
        assert!(attack_hits_enemy(
            &transform,
            AttackKind::Slash,
            Vec2::new(80.0, 0.0),
            24.0
        ));
        assert!(!attack_hits_enemy(
            &transform,
            AttackKind::Slash,
            Vec2::new(90.0, 0.0),
            24.0
        ));
        assert!(attack_hits_enemy(
            &transform,
            AttackKind::Fireball,
            Vec2::new(40.0, 0.0),
            24.0
        ));
        assert!(!attack_hits_enemy(
            &transform,
            AttackKind::Laser,
            Vec2::new(0.0, 70.0),
            10.0
        ));
    }

    #[test]
    fn g502_buttons_map_to_the_requested_abilities() {
        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Forward);
        assert!(matches!(mouse_attack_kind(&mouse), Some(AttackKind::Laser)));

        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Back);
        assert!(matches!(
            mouse_attack_kind(&mouse),
            Some(AttackKind::Fireball)
        ));

        let mut mouse = ButtonInput::default();
        mouse.press(MouseButton::Right);
        assert!(matches!(mouse_attack_kind(&mouse), Some(AttackKind::Slash)));
    }

    #[test]
    fn potion_use_consumes_one_item_and_respects_caps() {
        let mut state = crate::GameState::default();
        state.health = 80;
        state.mana = 70;
        state.inventory = vec![
            ItemKind::HealthPotion,
            ItemKind::ManaPotion,
            ItemKind::SpeedPotion,
        ];
        assert!(consume_item(&mut state, ItemKind::HealthPotion));
        assert!(consume_item(&mut state, ItemKind::ManaPotion));
        assert!(consume_item(&mut state, ItemKind::SpeedPotion));
        assert!(!consume_item(&mut state, ItemKind::HealthPotion));
        assert_eq!(
            (state.health, state.mana, state.speed_boost),
            (100, 100, 25.0)
        );
        assert!(state.inventory.is_empty());
    }

    #[test]
    fn pickup_collection_stops_at_twenty_four_slots() {
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Playing;
        state.inventory = vec![ItemKind::ManaPotion; 23];
        let mut app = App::new();
        app.insert_resource(state)
            .add_systems(Update, collect_pickups);
        app.world_mut().spawn((
            Player {
                hit_flash: 0.0,
                health: 100,
                max_health: 100,
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        let near = app
            .world_mut()
            .spawn((
                Pickup {
                    item: ItemKind::HealthPotion,
                },
                Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().resource::<crate::GameState>().inventory.len(),
            24
        );
        assert!(app.world().get_entity(near).is_none());
    }
}
