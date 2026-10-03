//! Shared health behavior for entities that can receive damage.
//!
//! The trait keeps combat code independent of whether its target is the
//! player or an enemy archetype.

/// Common health operations for combat targets.
pub trait Damageable {
    /// Returns the current health value.
    fn health(&self) -> i32;
    /// Returns the target's maximum health.
    fn max_health(&self) -> i32;
    /// Applies damage, clamping health at zero.
    fn take_damage(&mut self, amount: i32);
    /// Reports whether the target still has health remaining.
    fn is_alive(&self) -> bool {
        self.health() > 0
    }
}

use crate::audio::{self, AudioCues};
use crate::enemy::{self, Enemy, EnemyKind};
use crate::items;
use crate::player::{attack_hits_enemy, Attack, AttackKind, BounceVelocity, Player};
use crate::projectable::{self, FxKind, FxMaterial, HostileProjectile};
use crate::wall::{circle_hits_wall, Wall};
use bevy::prelude::*;

/// Advances player attacks, resolves enemy damage, and opens cleared-room rewards.
pub(crate) fn animate_attacks(
    time: Res<Time>,
    mut attacks: Query<(Entity, &mut Transform, &mut Attack), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Attack>>,
    walls: Query<(&Transform, &Wall), (Without<Attack>, Without<Enemy>)>,
    mut commands: Commands,
    mut state: ResMut<crate::GameState>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut fx_materials: Option<ResMut<Assets<FxMaterial>>>,
    cues: Option<Res<AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let mut killed_rooms = Vec::new();
    for (attack_entity, mut transform, mut attack) in &mut attacks {
        let next_position =
            transform.translation + attack.velocity.extend(0.0) * time.delta_seconds();
        if attack.kind != AttackKind::Slash {
            let radius = if attack.kind == AttackKind::Laser {
                6.0
            } else {
                18.0
            };
            if walls.iter().any(|(wall_transform, wall)| {
                circle_hits_wall(
                    next_position.truncate(),
                    radius,
                    wall_transform.translation.truncate(),
                    wall.size,
                )
            }) {
                commands.entity(attack_entity).despawn_recursive();
                continue;
            }
        }
        transform.translation = next_position;
        attack.age += time.delta_seconds();
        if attack.kind == AttackKind::Slash {
            transform.rotation = Quat::from_rotation_z(
                attack.rotation_offset + attack.age * 12.0 * attack.animation_speed,
            );
            transform.scale = Vec3::splat(1.0 + attack.age * 2.0);
        }
        let mut hit_enemy = false;
        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            let can_hit = (attack.kind != AttackKind::Slash
                || attack.age < 0.12 + time.delta_seconds())
                && !attack.hit_targets.contains(&enemy_entity)
                && attack_hits_enemy(
                    &transform,
                    attack.kind,
                    enemy_transform.translation.truncate(),
                    enemy.radius,
                );
            if can_hit {
                attack.hit_targets.push(enemy_entity);
                enemy.take_damage(attack.damage);
                enemy.hit_flash = 0.225;
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut())
                {
                    projectable::spawn_fx(
                        &mut commands,
                        meshes,
                        fx_materials,
                        FxKind::EnemyDamage,
                        enemy_transform.translation + Vec3::new(0.0, 0.0, 8.0),
                        Vec2::splat(enemy.radius * 4.2),
                        0.38,
                    );
                }
                if !enemy.is_alive() {
                    let drop = match enemy.kind {
                        EnemyKind::Minion => items::ItemKind::ManaPotion,
                        EnemyKind::Thug => items::ItemKind::HealthPotion,
                        EnemyKind::Miniboss => items::ItemKind::AttackSpeedPotion,
                    };
                    killed_rooms.push(enemy.room);
                    commands.spawn((
                        crate::room::RoomEntity,
                        items::Pickup { item: drop },
                        SpriteBundle {
                            sprite: Sprite {
                                color: items::item_color(drop),
                                custom_size: Some(Vec2::splat(22.0)),
                                ..default()
                            },
                            transform: *enemy_transform,
                            ..default()
                        },
                    ));
                    commands.entity(enemy_entity).despawn_recursive();
                }
                if attack.kind != AttackKind::Slash {
                    hit_enemy = true;
                }
                break;
            }
        }
        if hit_enemy {
            audio::play_sound(&mut commands, cues.as_deref(), audio::SoundKind::Hit);
            commands.entity(attack_entity).despawn_recursive();
        }
    }
    killed_rooms.sort_by_key(|room| (room.x, room.y));
    killed_rooms.dedup();
    for room in killed_rooms {
        if !enemies
            .iter()
            .any(|(_, enemy, _)| enemy.room == room && enemy.health > 0)
        {
            state.rooms.insert(room, true);
            state.room_notice = 3.0;
            state.upgrade_choices = crate::upgrades::upgrade_choices(room);
            state.chest_open_progress = 0.0;
            state.mode = crate::GameMode::Upgrade;
        }
    }
}

/// Advances hostile projectiles, stopping them on walls or the player.
pub(crate) fn hostile_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut state: ResMut<crate::GameState>,
    mut projectiles: Query<(Entity, &mut Transform, &HostileProjectile), Without<Player>>,
    walls: Query<(&Transform, &Wall), (Without<HostileProjectile>, Without<Player>)>,
    mut player: Query<(&mut Transform, &mut BounceVelocity, &mut Player)>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut fx_materials: Option<ResMut<Assets<FxMaterial>>>,
    cues: Option<Res<AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok((player_transform, mut player_bounce, mut player_data)) = player.get_single_mut() else {
        return;
    };
    for (entity, mut transform, projectile) in &mut projectiles {
        let next = transform.translation + projectile.velocity.extend(0.0) * time.delta_seconds();
        if walls.iter().any(|(wall_transform, wall)| {
            circle_hits_wall(
                next.truncate(),
                projectile.radius,
                wall_transform.translation.truncate(),
                wall.size,
            )
        }) {
            if let Some(effect_kind) = projectable::hostile_impact_effect(projectile.kind) {
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut())
                {
                    projectable::spawn_fx(
                        &mut commands,
                        meshes,
                        fx_materials,
                        effect_kind,
                        next.with_z(6.0),
                        Vec2::splat(projectile.radius * 3.4),
                        0.35,
                    );
                }
            }
            commands.entity(entity).despawn_recursive();
            continue;
        }
        transform.translation = next;
        if transform
            .translation
            .truncate()
            .distance(player_transform.translation.truncate())
            < projectile.radius + 19.0
        {
            let normal = (player_transform.translation - transform.translation)
                .truncate()
                .normalize_or_zero();
            player_bounce.0 = normal * 190.0 * state.knockback_multiplier;
            player_data.hit_flash = 0.225;
            if state.damage_cooldown <= 0.0 {
                player_data.take_damage(projectile.damage);
                state.health = player_data.health;
                state.damage_cooldown = 0.55;
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut())
                {
                    projectable::spawn_fx(
                        &mut commands,
                        meshes,
                        fx_materials,
                        FxKind::PlayerDamage,
                        player_transform.translation + Vec3::new(0.0, 0.0, 8.0),
                        Vec2::splat(88.0),
                        0.46,
                    );
                }
                if !player_data.is_alive() {
                    state.mode = crate::GameMode::Dead;
                }
            }
            if let Some(effect_kind) = projectable::hostile_impact_effect(projectile.kind) {
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut())
                {
                    projectable::spawn_fx(
                        &mut commands,
                        meshes,
                        fx_materials,
                        effect_kind,
                        transform.translation.with_z(6.0),
                        Vec2::splat(projectile.radius * 3.4),
                        0.35,
                    );
                }
            }
            audio::play_sound(&mut commands, cues.as_deref(), audio::SoundKind::Hit);
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Keeps the ECS player's health synchronized with the authoritative run state.
pub(crate) fn sync_player_damageable(state: Res<crate::GameState>, mut player: Query<&mut Player>) {
    let Ok(mut player) = player.get_single_mut() else {
        return;
    };
    player.max_health = state.max_health;
    player.health = state.health;
}

/// Separates player and enemies, applies contact damage, and imparts opposite impulses.
pub(crate) fn enemy_player_collision(
    mut player: Query<(&mut Transform, &mut BounceVelocity, &mut Player)>,
    mut enemies: Query<(&mut Transform, &mut BounceVelocity, &Enemy), Without<Player>>,
    mut state: ResMut<crate::GameState>,
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut fx_materials: Option<ResMut<Assets<FxMaterial>>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok((mut player, mut player_bounce, mut player_data)) = player.get_single_mut() else {
        return;
    };
    for (mut enemy, mut enemy_bounce, enemy_data) in &mut enemies {
        let delta = player.translation.truncate() - enemy.translation.truncate();
        let distance = delta.length();
        let minimum_distance = 19.0 + enemy_data.radius;
        if distance < minimum_distance {
            let normal = if distance > 0.001 {
                delta / distance
            } else {
                Vec2::Y
            };
            let overlap = minimum_distance - distance + 2.0;
            player.translation += (normal * overlap * 0.5).extend(0.0);
            enemy.translation -= (normal * overlap * 0.5).extend(0.0);
            player_bounce.0 = normal * 260.0 * state.knockback_multiplier;
            enemy_bounce.0 = -normal * 220.0 * state.knockback_multiplier;
            player_data.hit_flash = 0.225;
            if state.damage_cooldown <= 0.0 {
                let contact_damage = match enemy_data.kind {
                    EnemyKind::Minion => enemy::minion::CONTACT_DAMAGE,
                    EnemyKind::Thug => enemy::thug::CONTACT_DAMAGE,
                    EnemyKind::Miniboss => enemy::miniboss::CONTACT_DAMAGE,
                };
                player_data.take_damage(contact_damage);
                state.health = player_data.health;
                state.damage_cooldown = 0.55;
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut())
                {
                    projectable::spawn_fx(
                        &mut commands,
                        meshes,
                        fx_materials,
                        FxKind::PlayerDamage,
                        player.translation + Vec3::new(0.0, 0.0, 8.0),
                        Vec2::splat(88.0),
                        0.46,
                    );
                }
                if !player_data.is_alive() {
                    state.mode = crate::GameMode::Dead;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_enemy() -> Enemy {
        Enemy {
            kind: EnemyKind::Minion,
            health: 18,
            room: IVec2::ZERO,
            radius: enemy::minion::RADIUS,
            phase: 0.0,
            path: Vec::new(),
            path_index: 0,
            planned_goal: None,
            repath_timer: 0.0,
            hit_flash: 0.0,
            base_color: Color::srgb(0.2, 0.9, 0.72),
            attack_timer: 99.0,
            summon_timer: 99.0,
        }
    }

    #[test]
    fn player_and_enemy_contact_separates_both_and_deals_damage() {
        let mut app = App::new();
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Playing;
        app.insert_resource(state)
            .add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((
            Player {
                hit_flash: 0.0,
                health: 100,
                max_health: 100,
            },
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world_mut().spawn((
            test_enemy(),
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::new(25.0, 0.0, 0.0)),
        ));
        app.update();
        let mut players = app
            .world_mut()
            .query::<(&Transform, &BounceVelocity, &Player)>();
        let (player, bounce, data) = players.single(app.world());
        assert!(player.translation.x < 0.0);
        assert!(bounce.0.x < 0.0);
        assert_eq!(data.health, 94);
    }

    #[test]
    fn contact_damage_respects_invulnerability_timer() {
        let mut app = App::new();
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Playing;
        state.damage_cooldown = 0.5;
        app.insert_resource(state)
            .add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((
            Player {
                hit_flash: 0.0,
                health: 100,
                max_health: 100,
            },
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world_mut().spawn((
            test_enemy(),
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::new(25.0, 0.0, 0.0)),
        ));
        app.update();
        assert_eq!(app.world().resource::<crate::GameState>().health, 100);
    }

    #[test]
    fn attacks_damage_once_flash_enemies_and_open_cleared_room_state() {
        let mut app = App::new();
        let mut state = crate::GameState::default();
        state.mode = crate::GameMode::Playing;
        app.insert_resource(state)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, animate_attacks);
        let room = IVec2::new(1, -1);
        let enemy = app
            .world_mut()
            .spawn((
                test_enemy(),
                Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
            ))
            .id();
        app.world_mut()
            .entity_mut(enemy)
            .get_mut::<Enemy>()
            .unwrap()
            .room = room;
        app.world_mut().spawn((
            Attack {
                kind: AttackKind::Laser,
                damage: 18,
                velocity: Vec2::ZERO,
                age: 0.0,
                hit_targets: Vec::new(),
                animation_speed: 1.0,
                rotation_offset: 0.0,
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        app.update();
        assert!(app.world().get::<Enemy>(enemy).is_none());
        assert_eq!(
            app.world().resource::<crate::GameState>().rooms.get(&room),
            Some(&true)
        );
        assert_eq!(
            app.world().resource::<crate::GameState>().mode,
            crate::GameMode::Upgrade
        );
    }
}
