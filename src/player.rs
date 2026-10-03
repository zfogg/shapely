//! Player state, movement, abilities, and player-facing combat helpers.
//!
//! The three abilities are represented by the same [`Attack`] component and
//! differ through [`AttackKind`] and [`AttackProfile`].

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use crate::damageable::Damageable;
use crate::projectable::{FxKind, FxMaterial};

#[derive(Component)]
/// The controllable square character and its damage state.
pub struct Player { pub hit_flash: f32, pub health: i32, pub max_health: i32 }

impl Damageable for Player {
    fn health(&self) -> i32 { self.health }
    fn max_health(&self) -> i32 { self.max_health }
    fn take_damage(&mut self, amount: i32) { self.health = (self.health - amount).max(0); }
}

#[derive(Component)]
/// A temporary velocity used for knockback impulses.
pub struct BounceVelocity(pub Vec2);

#[derive(Component)]
/// A player attack moving through the room or animating in place.
pub struct Attack { pub kind: AttackKind, pub damage: i32, pub velocity: Vec2, pub age: f32, pub hit_targets: Vec<Entity> }

#[derive(Clone, Copy, PartialEq)]
/// The player's three ability types.
pub enum AttackKind { Laser, Slash, Fireball }

/// Tunable combat values for one ability.
pub struct AttackProfile { pub damage: i32, pub speed: f32, pub max_age: f32, pub size: Vec2, pub mana_cost: i32, pub cooldown: f32 }

/// Returns the design profile for an ability.
pub fn attack_profile(kind: AttackKind) -> AttackProfile {
    match kind {
        AttackKind::Laser => AttackProfile { damage: 5, speed: 900.0, max_age: 1.2, size: Vec2::new(130.0, 8.0), mana_cost: 8, cooldown: 0.25 },
        AttackKind::Slash => AttackProfile { damage: 8, speed: 0.0, max_age: 0.22, size: Vec2::splat(120.0), mana_cost: 0, cooldown: 0.45 },
        AttackKind::Fireball => AttackProfile { damage: 7, speed: 330.0, max_age: 1.5, size: Vec2::splat(34.0), mana_cost: 8, cooldown: 0.8 },
    }
}

/// Tests whether an attack overlaps any part of an enemy shape.
pub fn attack_hits_enemy(attack_transform: &Transform, kind: AttackKind, enemy_position: Vec2, enemy_radius: f32) -> bool {
    let relative = enemy_position - attack_transform.translation.truncate();
    match kind {
        AttackKind::Slash => relative.length() <= 60.0 + enemy_radius,
        AttackKind::Fireball => relative.length() <= 18.0 + enemy_radius,
        AttackKind::Laser => {
            let angle = attack_transform.rotation.to_euler(EulerRot::ZYX).0;
            let local = Vec2::new(relative.x * angle.cos() + relative.y * angle.sin(), -relative.x * angle.sin() + relative.y * angle.cos());
            local.x.abs() <= 65.0 + enemy_radius && local.y.abs() <= 4.0 + enemy_radius
        }
    }
}

/// Moves the player while applying active knockback and speed boosts.
pub fn player_movement(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, state: Res<crate::GameState>, mut query: Query<(&mut Transform, &mut BounceVelocity), With<Player>>) {
    if state.mode != crate::GameMode::Playing { return; }
    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { direction.y += 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { direction.y -= 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { direction.x -= 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { direction.x += 1.0; }
    if let Ok((mut transform, mut bounce)) = query.get_single_mut() {
        if bounce.0.length_squared() > 1.0 { transform.translation += bounce.0.extend(0.0) * time.delta_seconds(); bounce.0 = bounce.0.lerp(Vec2::ZERO, (10.0 * time.delta_seconds()).min(1.0)); return; }
        let speed = crate::PLAYER_SPEED * if state.speed_boost > 0.0 { 1.55 } else { 1.0 };
        transform.translation += direction.normalize_or_zero().extend(0.0) * speed * time.delta_seconds();
        transform.translation.x = transform.translation.x.clamp(-570.0, 570.0);
        transform.translation.y = transform.translation.y.clamp(-320.0, 320.0);
    }
}

/// Reads keyboard ability input and aims attacks at the mouse cursor.
pub fn attack_input(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut state: ResMut<crate::GameState>, player: Query<&Transform, With<Player>>, windows: Query<&Window>, cameras: Query<(&Camera, &GlobalTransform)>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<FxMaterial>>) {
    if state.mode != crate::GameMode::Playing { return; }
    let Ok(player) = player.get_single() else { return };
    let kind = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Space) { Some(AttackKind::Laser) } else if keys.just_pressed(KeyCode::Digit2) { Some(AttackKind::Slash) } else if keys.just_pressed(KeyCode::Digit3) { Some(AttackKind::Fireball) } else { None };
    let Some(kind) = kind else { return };
    let index = match kind { AttackKind::Laser => 0, AttackKind::Slash => 1, AttackKind::Fireball => 2 };
    if state.cooldowns[index] > 0.0 { return; }
    let profile = attack_profile(kind);
    if state.mana < profile.mana_cost { return; }
    state.mana -= profile.mana_cost;
    let (camera, camera_transform) = cameras.single();
    let Some(cursor) = windows.single().cursor_position().and_then(|position| camera.viewport_to_world_2d(camera_transform, position)) else { return };
    let direction = (cursor - player.translation.truncate()).normalize_or_zero();
    if direction == Vec2::ZERO { return; }
    state.cooldowns[index] = profile.cooldown * state.cooldown_multiplier;
    let spawn_position = player.translation + direction.extend(0.0) * 42.0 + Vec3::new(0.0, 0.0, 4.0);
    let fx_kind = match kind { AttackKind::Laser => FxKind::Laser, AttackKind::Slash => FxKind::Slash, AttackKind::Fireball => FxKind::Fireball };
    let mesh = if kind == AttackKind::Fireball { meshes.add(Circle::new(profile.size.x * 0.5)).into() } else { meshes.add(Rectangle::new(profile.size.x, profile.size.y)).into() };
    commands.spawn((Attack { kind, damage: profile.damage, velocity: direction * profile.speed, age: 0.0, hit_targets: Vec::new() }, crate::Lifetime(profile.max_age), MaterialMesh2dBundle { mesh, material: materials.add(FxMaterial::new(fx_kind, 0.0, 1.0)), transform: Transform { translation: spawn_position, rotation: Quat::from_rotation_z(direction.y.atan2(direction.x)), scale: Vec3::new(state.projectile_scale, state.projectile_scale, 1.0) }, ..default() }));
    let _ = time;
}

/// Advances the player's damage flash and restores its normal color.
pub fn player_flash(time: Res<Time>, state: Res<crate::GameState>, mut player: Query<(&mut Player, &mut Sprite)>) {
    if state.mode == crate::GameMode::Title { return; }
    let Ok((mut player, mut sprite)) = player.get_single_mut() else { return };
    player.hit_flash = (player.hit_flash - time.delta_seconds()).max(0.0);
    sprite.color = if player.hit_flash > 0.0 { Color::BLACK } else { Color::srgb(1.0, 0.12, 0.4) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_profiles_match_damage_and_resource_design() {
        let laser = attack_profile(AttackKind::Laser);
        assert_eq!((laser.damage, laser.speed, laser.mana_cost), (5, 900.0, 8));
        let slash = attack_profile(AttackKind::Slash);
        assert_eq!((slash.damage, slash.speed, slash.mana_cost), (8, 0.0, 0));
        let fireball = attack_profile(AttackKind::Fireball);
        assert_eq!((fireball.damage, fireball.speed, fireball.mana_cost), (7, 330.0, 8));
    }

    #[test]
    fn hitboxes_use_enemy_edges_and_attack_direction() {
        let transform = Transform::from_translation(Vec3::ZERO);
        assert!(attack_hits_enemy(&transform, AttackKind::Slash, Vec2::new(80.0, 0.0), 24.0));
        assert!(!attack_hits_enemy(&transform, AttackKind::Slash, Vec2::new(90.0, 0.0), 24.0));
        assert!(attack_hits_enemy(&transform, AttackKind::Fireball, Vec2::new(40.0, 0.0), 24.0));
        assert!(!attack_hits_enemy(&transform, AttackKind::Laser, Vec2::new(0.0, 70.0), 10.0));
    }
}
