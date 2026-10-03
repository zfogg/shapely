//! Enemy components, navigation behavior, attack patterns, and archetype data.
//!
//! Each concrete enemy type has a small child module containing its balance
//! constants, while this module owns shared ECS behavior.

use crate::damageable::Damageable;
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;

pub mod miniboss;
pub mod minion;
pub mod thug;

#[derive(Component)]
/// Shared runtime state for every enemy archetype.
pub struct Enemy {
    pub kind: EnemyKind,
    pub health: i32,
    pub room: IVec2,
    pub radius: f32,
    pub phase: f32,
    pub path: Vec<crate::navigation::GridPos>,
    pub path_index: usize,
    pub planned_goal: Option<crate::navigation::GridPos>,
    pub repath_timer: f32,
    pub hit_flash: f32,
    pub base_color: Color,
    pub attack_timer: f32,
    pub summon_timer: f32,
}

/// Persisted enemy state while the player is visiting another room.
#[derive(Clone)]
pub(crate) struct EnemySnapshot {
    pub(crate) kind: EnemyKind,
    pub(crate) health: i32,
    pub(crate) position: Vec3,
    pub(crate) phase: f32,
    pub(crate) attack_timer: f32,
    pub(crate) summon_timer: f32,
}

#[derive(Clone, Copy)]
/// Enemy archetypes available in Shapely rooms.
pub enum EnemyKind {
    Minion,
    Thug,
    Miniboss,
}

/// Returns `(health, radius, movement speed)` for an archetype.
pub fn spec(kind: EnemyKind) -> (i32, f32, f32) {
    match kind {
        EnemyKind::Minion => (minion::HEALTH, minion::RADIUS, minion::MOVE_SPEED),
        EnemyKind::Thug => (thug::HEALTH, thug::RADIUS, thug::MOVE_SPEED),
        EnemyKind::Miniboss => (miniboss::HEALTH, miniboss::RADIUS, miniboss::MOVE_SPEED),
    }
}

/// Populates a room with its archetype set.
pub(crate) fn spawn_enemies(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    room: IVec2,
    font: Handle<Font>,
) {
    let shift = (crate::room::seed(room) % 5) as f32 * 18.0;
    let specs = [
        (EnemyKind::Minion, Vec2::new(-260.0 + shift, 120.0)),
        (EnemyKind::Minion, Vec2::new(250.0 - shift, -80.0)),
        (EnemyKind::Thug, Vec2::new(180.0, 180.0 - shift)),
        (EnemyKind::Miniboss, Vec2::new(0.0, -160.0 + shift)),
    ];
    for (kind, pos) in specs {
        spawn_enemy(commands, meshes, materials, kind, pos, room, font.clone());
    }
}

/// Spawns one enemy and its health/name presentation children.
pub(crate) fn spawn_enemy(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    kind: EnemyKind,
    pos: Vec2,
    room: IVec2,
    font: Handle<Font>,
) -> Entity {
    let (hp, radius, _) = spec(kind);
    let (mesh, color) = match kind {
        EnemyKind::Minion => (meshes.add(Circle::new(radius)), Color::srgb(0.2, 0.9, 0.72)),
        EnemyKind::Thug => (
            meshes.add(RegularPolygon::new(radius, 6)),
            Color::srgb(1.0, 0.48, 0.16),
        ),
        EnemyKind::Miniboss => (
            meshes.add(RegularPolygon::new(radius, 8)),
            Color::srgb(0.78, 0.25, 1.0),
        ),
    };
    let entity = commands
        .spawn((
            crate::RoomEntity,
            crate::player::BounceVelocity(Vec2::ZERO),
            Enemy {
                kind,
                health: hp,
                room,
                radius,
                phase: pos.x * 0.01,
                path: Vec::new(),
                path_index: 0,
                planned_goal: None,
                repath_timer: 0.0,
                hit_flash: 0.0,
                base_color: color,
                attack_timer: 1.5,
                summon_timer: 4.0,
            },
            MaterialMesh2dBundle {
                mesh: mesh.into(),
                material: materials.add(ColorMaterial::from(color)),
                transform: Transform::from_xyz(pos.x, pos.y, 3.0),
                ..default()
            },
        ))
        .id();
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            crate::ui::EnemyHealthBar { max_health: hp },
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(0.2, 0.95, 0.45),
                    custom_size: Some(Vec2::new(radius * 2.0, 5.0)),
                    ..default()
                },
                transform: Transform::from_xyz(0.0, radius + 10.0, 1.0),
                ..default()
            },
        ));
        if matches!(kind, EnemyKind::Miniboss) {
            parent.spawn((
                crate::ui::EnemyName,
                Text2dBundle {
                    text: Text::from_section(
                        "MINIBOSS",
                        TextStyle {
                            font,
                            font_size: 15.0,
                            color: Color::srgb(1.0, 0.55, 0.85),
                        },
                    ),
                    transform: Transform::from_xyz(0.0, radius + 21.0, 1.0),
                    ..default()
                },
            ));
        }
    });
    entity
}

impl Damageable for Enemy {
    fn health(&self) -> i32 {
        self.health
    }
    fn max_health(&self) -> i32 {
        spec(self.kind).0
    }
    fn take_damage(&mut self, amount: i32) {
        self.health = (self.health - amount).max(0);
    }
}

/// Updates enemy hit flashes and restores their archetype color.
pub fn enemy_flash(
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut enemies: Query<(&mut Enemy, &Handle<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    for (mut enemy, material_handle) in &mut enemies {
        enemy.hit_flash = (enemy.hit_flash - time.delta_seconds()).max(0.0);
        if let Some(material) = materials.get_mut(material_handle) {
            material.color = if enemy.hit_flash > 0.0 {
                Color::srgb(1.0, 0.03, 0.03)
            } else {
                enemy.base_color
            };
        }
    }
}

/// Repaths enemies through the room grid toward the player.
pub fn enemy_ai(
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut enemy_queries: ParamSet<(
        Query<
            (
                Entity,
                &mut Transform,
                &mut Enemy,
                &mut crate::player::BounceVelocity,
            ),
            Without<crate::player::Player>,
        >,
        Query<(Entity, &Transform, &Enemy), With<Enemy>>,
    )>,
    player: Query<&Transform, (With<crate::player::Player>, Without<Enemy>)>,
    nav: Res<crate::navigation::RoomNavGrid>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(player) = player.get_single() else {
        return;
    };
    let Some(goal) = nav.nearest_open(nav.world_to_cell(player.translation.truncate())) else {
        return;
    };
    let occupied_enemies: Vec<(Entity, Vec2, f32)> = enemy_queries
        .p1()
        .iter()
        .map(|(entity, transform, enemy)| (entity, transform.translation.truncate(), enemy.radius))
        .collect();
    for (entity, mut transform, mut enemy, mut bounce) in &mut enemy_queries.p0() {
        enemy.phase += time.delta_seconds();
        if bounce.0.length_squared() > 1.0 {
            transform.translation += bounce.0.extend(0.0) * time.delta_seconds();
            bounce.0 = bounce
                .0
                .lerp(Vec2::ZERO, (10.0 * time.delta_seconds()).min(1.0));
            continue;
        }
        let to_player = (player.translation - transform.translation).truncate();
        if to_player.length() > enemy.radius + 19.0 {
            let start = nav.nearest_open(nav.world_to_cell(transform.translation.truncate()));
            enemy.repath_timer = (enemy.repath_timer - time.delta_seconds()).max(0.0);
            if enemy.planned_goal != Some(goal)
                || enemy.path_index >= enemy.path.len()
                || enemy.repath_timer <= 0.0
            {
                if let Some(start) = start {
                    if let Some(path) = crate::navigation::find_enemy_path(
                        &nav,
                        start,
                        goal,
                        entity,
                        &occupied_enemies,
                    ) {
                        enemy.path = path;
                        enemy.path_index = 1;
                        enemy.planned_goal = Some(goal);
                        enemy.repath_timer = 0.2;
                    }
                }
            }
            if enemy.path_index < enemy.path.len() {
                let waypoint = nav.cell_to_world(enemy.path[enemy.path_index]);
                let current = transform.translation.truncate();
                transform.translation += (waypoint - current).normalize_or_zero().extend(0.0)
                    * spec(enemy.kind).2
                    * time.delta_seconds();
                if transform.translation.truncate().distance(waypoint) < 8.0 {
                    enemy.path_index += 1;
                }
            }
        }
        transform.rotation = Quat::from_rotation_z((enemy.phase * 2.0).sin() * 0.08);
    }
}

/// Runs minion charges, thug shots, and miniboss radial attacks/summons.
pub fn enemy_attacks(
    time: Res<Time>,
    mut commands: Commands,
    state: Res<crate::GameState>,
    player: Query<&Transform, (With<crate::player::Player>, Without<Enemy>)>,
    mut enemies: Query<(&Transform, &mut Enemy)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fx_materials: ResMut<Assets<crate::projectable::FxMaterial>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    assets: Res<AssetServer>,
    cues: Option<Res<crate::audio::AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(player) = player.get_single() else {
        return;
    };
    for (transform, mut enemy) in &mut enemies {
        enemy.attack_timer = (enemy.attack_timer - time.delta_seconds()).max(0.0);
        enemy.summon_timer = (enemy.summon_timer - time.delta_seconds()).max(0.0);
        let origin = transform.translation.truncate();
        let direction = (player.translation.truncate() - origin).normalize_or_zero();
        if enemy.attack_timer <= 0.0 {
            match enemy.kind {
                EnemyKind::Minion => enemy.attack_timer = minion::ATTACK_COOLDOWN,
                EnemyKind::Thug => {
                    crate::projectable::spawn_hostile_projectile(
                        &mut commands,
                        &mut meshes,
                        &mut fx_materials,
                        crate::projectable::FxKind::ThugShot,
                        origin,
                        direction,
                        thug::PROJECTILE_SPEED,
                        thug::PROJECTILE_DAMAGE,
                        12.0,
                    );
                    enemy.attack_timer = thug::ATTACK_COOLDOWN;
                    crate::audio::play_sound(
                        &mut commands,
                        cues.as_deref(),
                        crate::audio::SoundKind::Attack,
                    );
                }
                EnemyKind::Miniboss => {
                    for index in 0..miniboss::RADIAL_PROJECTILES {
                        let angle = index as f32 * std::f32::consts::TAU
                            / miniboss::RADIAL_PROJECTILES as f32;
                        crate::projectable::spawn_hostile_projectile(
                            &mut commands,
                            &mut meshes,
                            &mut fx_materials,
                            crate::projectable::FxKind::MinibossBurst,
                            origin,
                            Vec2::from_angle(angle),
                            185.0,
                            miniboss::PROJECTILE_DAMAGE,
                            11.0,
                        );
                    }
                    enemy.attack_timer = miniboss::ATTACK_COOLDOWN;
                    crate::audio::play_sound(
                        &mut commands,
                        cues.as_deref(),
                        crate::audio::SoundKind::Attack,
                    );
                }
            }
        }
        if matches!(enemy.kind, EnemyKind::Miniboss) && enemy.summon_timer <= 0.0 {
            spawn_enemy(
                &mut commands,
                &mut meshes,
                &mut color_materials,
                EnemyKind::Minion,
                origin + direction.perp() * 72.0,
                enemy.room,
                assets.load("fonts/FiraSans-Bold.ttf"),
            );
            enemy.summon_timer = miniboss::SUMMON_COOLDOWN;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archetypes_have_distinct_stats() {
        assert_eq!(spec(EnemyKind::Minion).0, minion::HEALTH);
        assert_eq!(spec(EnemyKind::Thug).1, thug::RADIUS);
        assert!(spec(EnemyKind::Miniboss).0 > spec(EnemyKind::Thug).0);
        assert_eq!(miniboss::RADIAL_PROJECTILES, 8);
    }

    #[test]
    fn damageable_enemy_reaches_zero_without_underflow() {
        let mut enemy = Enemy {
            kind: EnemyKind::Minion,
            health: 18,
            room: IVec2::ZERO,
            radius: minion::RADIUS,
            phase: 0.0,
            path: Vec::new(),
            path_index: 0,
            planned_goal: None,
            repath_timer: 0.0,
            hit_flash: 0.0,
            base_color: Color::WHITE,
            attack_timer: 0.0,
            summon_timer: 0.0,
        };
        enemy.take_damage(100);
        assert_eq!(enemy.health(), 0);
        assert!(!enemy.is_alive());
    }

    #[test]
    fn enemy_attack_damage_scales_from_minion_to_thug_to_miniboss() {
        assert!(minion::CONTACT_DAMAGE < thug::CONTACT_DAMAGE);
        assert!(thug::PROJECTILE_DAMAGE < miniboss::PROJECTILE_DAMAGE);
        assert!(thug::CONTACT_DAMAGE < miniboss::CONTACT_DAMAGE);
    }
}
