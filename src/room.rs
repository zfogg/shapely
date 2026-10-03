//! Room geometry constants and deterministic room identity helpers.

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;

/// World-space dimensions of every room.
pub const ROOM_SIZE: Vec2 = Vec2::new(1120.0, 620.0);

/// Marks entities that belong to the currently spawned room.
#[derive(Component)]
pub(crate) struct RoomEntity;

/// Deterministic enemy composition for one room in the 3x3 world grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RoomPopulation {
    pub(crate) minions: usize,
    pub(crate) thugs: usize,
    pub(crate) minibosses: usize,
}

/// Returns the enemy mix for a room, keeping elite encounters uncommon.
pub(crate) fn room_population(room: IVec2) -> RoomPopulation {
    let column = (room.x + 1).rem_euclid(3);
    let row = (room.y + 1).rem_euclid(3);
    let room_index = column + row * 3;
    // Two rooms are miniboss rooms; three additional rooms are thug-only,
    // for five thug rooms total.
    let miniboss_room = matches!(room_index, 0 | 8);
    let thug_room = miniboss_room || matches!(room_index, 1 | 3 | 5);

    RoomPopulation {
        // Most rooms have minions, but one ordinary room is a sparse elite
        // encounter and miniboss rooms stay intentionally less crowded.
        minions: if room_index == 4 {
            0
        } else if miniboss_room {
            1
        } else if thug_room {
            2
        } else {
            3
        },
        thugs: if miniboss_room {
            2
        } else if thug_room {
            1
        } else {
            0
        },
        minibosses: usize::from(miniboss_room),
    }
}

/// Per-room persistence for enemies and hostile projectiles.
#[derive(Resource, Default)]
pub(crate) struct RoomSnapshots {
    pub(crate) enemies: std::collections::HashMap<IVec2, Vec<crate::enemy::EnemySnapshot>>,
    pub(crate) projectiles:
        std::collections::HashMap<IVec2, Vec<crate::projectable::ProjectileSnapshot>>,
}

/// Produces a stable pseudo-random seed for a room coordinate.
pub fn seed(room: IVec2) -> u32 {
    let x = room.x as u32;
    let y = room.y as u32;
    x.wrapping_mul(0x9E37_79B9).rotate_left(13)
        ^ y.wrapping_mul(0x85EB_CA6B).rotate_right(7)
        ^ 0xC0FF_EE11
}

/// Builds deterministic walls and openings for one room in the 3x3 grid.
pub(crate) fn room_walls(room: IVec2) -> Vec<(Vec2, Vec2)> {
    let seed = seed(room);
    let mut walls = Vec::new();
    if room.y < 1 {
        walls.push((Vec2::new(-350.0, 305.0), Vec2::new(410.0, 12.0)));
        walls.push((Vec2::new(350.0, 305.0), Vec2::new(410.0, 12.0)));
    } else {
        walls.push((Vec2::new(0.0, 305.0), Vec2::new(1120.0, 12.0)));
    }
    if room.y > -1 {
        walls.push((Vec2::new(-350.0, -305.0), Vec2::new(410.0, 12.0)));
        walls.push((Vec2::new(350.0, -305.0), Vec2::new(410.0, 12.0)));
    } else {
        walls.push((Vec2::new(0.0, -305.0), Vec2::new(1120.0, 12.0)));
    }
    if room.x > -1 {
        walls.push((Vec2::new(-555.0, -150.0), Vec2::new(12.0, 290.0)));
        walls.push((Vec2::new(-555.0, 225.0), Vec2::new(12.0, 140.0)));
    } else {
        walls.push((Vec2::new(-555.0, 0.0), Vec2::new(12.0, 620.0)));
    }
    if room.x < 1 {
        walls.push((Vec2::new(555.0, -150.0), Vec2::new(12.0, 290.0)));
        walls.push((Vec2::new(555.0, 225.0), Vec2::new(12.0, 140.0)));
    } else {
        walls.push((Vec2::new(555.0, 0.0), Vec2::new(12.0, 620.0)));
    }
    let wall_count = match seed % 4 {
        0 => 1,
        1 => 3,
        2 => 5,
        _ => 7,
    };
    for i in 0..wall_count {
        let value = seed.wrapping_add(i * 0x45D9_F3B);
        let x = -360.0 + ((value % 7) as f32) * 120.0;
        let y = -190.0 + (((value / 7) % 5) as f32) * 95.0;
        let horizontal_wall = value & 1 == 0;
        let size = if horizontal_wall {
            Vec2::new(150.0 + ((value / 13) % 2) as f32 * 70.0, 12.0)
        } else {
            Vec2::new(12.0, 115.0 + ((value / 17) % 2) as f32 * 55.0)
        };
        walls.push((Vec2::new(x, y), size));
    }
    walls
}

/// Spawns the room floor, walls, accents, navigation grid, and optionally enemies.
pub(crate) fn build_room(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    room: IVec2,
    populated: bool,
    mut nav: ResMut<crate::navigation::RoomNavGrid>,
    font: Handle<Font>,
) {
    let seed = seed(room);
    commands.spawn((
        RoomEntity,
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgb(
                    0.035 + (seed % 4) as f32 * 0.006,
                    0.045,
                    0.12 + (seed % 3) as f32 * 0.008,
                ),
                custom_size: Some(ROOM_SIZE),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, -2.0),
            ..default()
        },
    ));
    let wall_color = Color::srgb(0.18, 0.22, 0.5);
    let walls = room_walls(room);
    for (position, size) in &walls {
        crate::wall::spawn_wall(
            commands,
            *position,
            *size,
            if size.x > size.y {
                wall_color
            } else {
                Color::srgb(0.12, 0.28, 0.48)
            },
        );
    }
    nav.rebuild(room, &walls);
    let accents = [
        (-380.0, 190.0),
        (360.0, 155.0),
        (-280.0, -190.0),
        (260.0, -190.0),
    ];
    for (index, (x, y)) in accents.iter().enumerate() {
        if (seed + index as u32) % 3 != 0 {
            commands.spawn((
                RoomEntity,
                MaterialMesh2dBundle {
                    mesh: meshes.add(Rectangle::new(90.0, 5.0)).into(),
                    material: materials.add(ColorMaterial::from(Color::srgb(0.06, 0.8, 0.92))),
                    transform: Transform::from_xyz(*x, *y, -1.0),
                    ..default()
                },
            ));
        }
    }
    if populated {
        crate::enemy::spawn_enemies(commands, meshes, materials, room, font);
    }
}

/// Restores enemies and hostile projectiles saved when the player left a room.
pub(crate) fn restore_room_snapshot(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    color_materials: &mut ResMut<Assets<ColorMaterial>>,
    fx_materials: &mut ResMut<Assets<crate::projectable::FxMaterial>>,
    snapshots: &RoomSnapshots,
    room: IVec2,
    font: Handle<Font>,
) {
    if let Some(enemies) = snapshots.enemies.get(&room) {
        for snapshot in enemies {
            let entity = crate::enemy::spawn_enemy(
                commands,
                meshes,
                color_materials,
                snapshot.kind,
                snapshot.position.truncate(),
                room,
                font.clone(),
            );
            let (_, radius, _) = crate::enemy::spec(snapshot.kind);
            commands.entity(entity).insert(crate::enemy::Enemy {
                kind: snapshot.kind,
                health: snapshot.health,
                room,
                radius,
                phase: snapshot.phase,
                path: Vec::new(),
                path_index: 0,
                planned_goal: None,
                repath_timer: 0.0,
                hit_flash: 0.0,
                base_color: match snapshot.kind {
                    crate::enemy::EnemyKind::Minion => Color::srgb(0.2, 0.9, 0.72),
                    crate::enemy::EnemyKind::Thug => Color::srgb(1.0, 0.48, 0.16),
                    crate::enemy::EnemyKind::Miniboss => Color::srgb(0.78, 0.25, 1.0),
                },
                attack_timer: snapshot.attack_timer,
                summon_timer: snapshot.summon_timer,
            });
        }
    }
    if let Some(projectiles) = snapshots.projectiles.get(&room) {
        for snapshot in projectiles {
            let direction = snapshot.velocity.normalize_or_zero();
            let speed = snapshot.velocity.length();
            let entity = crate::projectable::spawn_hostile_projectile(
                commands,
                meshes,
                fx_materials,
                snapshot.kind,
                snapshot.position.truncate(),
                if direction == Vec2::ZERO {
                    Vec2::X
                } else {
                    direction
                },
                speed,
                snapshot.damage,
                snapshot.radius,
            );
            commands.entity(entity).insert((
                Transform::from_translation(snapshot.position),
                crate::projectable::HostileProjectile {
                    velocity: snapshot.velocity,
                    damage: snapshot.damage,
                    radius: snapshot.radius,
                    kind: snapshot.kind,
                },
                crate::Lifetime(snapshot.lifetime),
            ));
        }
    }
}

/// Changes rooms through valid wall openings and preserves each room's state.
pub(crate) fn update_room(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<crate::GameState>,
    mut snapshots: ResMut<RoomSnapshots>,
    mut player: Query<&mut Transform, With<crate::player::Player>>,
    enemies: Query<
        (&crate::enemy::Enemy, &Transform),
        (
            With<crate::enemy::Enemy>,
            Without<crate::player::Player>,
            Without<crate::projectable::HostileProjectile>,
        ),
    >,
    projectiles: Query<
        (
            &crate::projectable::HostileProjectile,
            &Transform,
            &crate::Lifetime,
        ),
        (
            With<crate::projectable::HostileProjectile>,
            Without<crate::player::Player>,
            Without<crate::enemy::Enemy>,
        ),
    >,
    old_room: Query<Entity, With<RoomEntity>>,
    projectile_entities: Query<Entity, With<crate::projectable::HostileProjectile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut fx_materials: ResMut<Assets<crate::projectable::FxMaterial>>,
    nav: ResMut<crate::navigation::RoomNavGrid>,
    assets: Res<AssetServer>,
    cues: Option<Res<crate::audio::AudioCues>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(mut player) = player.get_single_mut() else {
        return;
    };
    let mut next = state.room;
    if player.translation.x < -548.0 {
        next.x -= 1;
        player.translation.x = 520.0;
    }
    if player.translation.x > 548.0 {
        next.x += 1;
        player.translation.x = -520.0;
    }
    if player.translation.y < -298.0 {
        next.y -= 1;
        player.translation.y = 270.0;
    }
    if player.translation.y > 298.0 {
        next.y += 1;
        player.translation.y = -270.0;
    }
    if next.x.abs() > 1 || next.y.abs() > 1 {
        player.translation.x = player.translation.x.clamp(-525.0, 525.0);
        player.translation.y = player.translation.y.clamp(-275.0, 275.0);
        return;
    }
    if next != state.room {
        let current_room = state.room;
        snapshots.enemies.insert(
            current_room,
            enemies
                .iter()
                .map(|(enemy, transform)| crate::enemy::EnemySnapshot {
                    kind: enemy.kind,
                    health: enemy.health,
                    position: transform.translation,
                    phase: enemy.phase,
                    attack_timer: enemy.attack_timer,
                    summon_timer: enemy.summon_timer,
                })
                .collect(),
        );
        snapshots.projectiles.insert(
            current_room,
            projectiles
                .iter()
                .map(
                    |(projectile, transform, lifetime)| crate::projectable::ProjectileSnapshot {
                        kind: projectile.kind,
                        position: transform.translation,
                        velocity: projectile.velocity,
                        damage: projectile.damage,
                        radius: projectile.radius,
                        lifetime: lifetime.0,
                    },
                )
                .collect(),
        );
        for entity in &old_room {
            commands.entity(entity).despawn_recursive();
        }
        for entity in &projectile_entities {
            commands.entity(entity).despawn_recursive();
        }
        state.room = next;
        let has_snapshot = snapshots.enemies.contains_key(&next);
        let populated = !*state.rooms.get(&next).unwrap_or(&false) && !has_snapshot;
        build_room(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            next,
            populated,
            nav,
            assets.load("fonts/FiraSans-Bold.ttf"),
        );
        restore_room_snapshot(
            &mut commands,
            &mut meshes,
            &mut color_materials,
            &mut fx_materials,
            &snapshots,
            next,
            assets.load("fonts/FiraSans-Bold.ttf"),
        );
        crate::audio::play_sound(
            &mut commands,
            cues.as_deref(),
            crate::audio::SoundKind::Transition,
        );
    }
    let _ = keys;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_seed_is_deterministic_and_varies_by_room() {
        assert_eq!(seed(IVec2::ZERO), seed(IVec2::ZERO));
        assert_ne!(seed(IVec2::ZERO), seed(IVec2::new(1, 0)));
    }

    #[test]
    fn center_room_has_openings_on_all_four_sides() {
        let walls = room_walls(IVec2::ZERO);
        assert!(walls.iter().filter(|(_, size)| size.y <= 12.0).count() >= 4);
        assert!(walls.iter().filter(|(_, size)| size.x <= 12.0).count() >= 4);
    }

    #[test]
    fn edge_rooms_have_solid_walls_toward_missing_neighbors() {
        let corner = room_walls(IVec2::new(1, 1));
        assert!(corner
            .iter()
            .any(|(position, size)| position.y > 300.0 && size.x > 1000.0));
        assert!(corner
            .iter()
            .any(|(position, size)| position.x > 550.0 && size.y > 600.0));
    }

    #[test]
    fn room_layouts_are_deterministic_but_not_all_identical() {
        let first = room_walls(IVec2::new(-1, 1));
        assert_eq!(first, room_walls(IVec2::new(-1, 1)));
        assert!((-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| IVec2::new(x, y)))
            .any(|room| room != IVec2::new(-1, 1) && room_walls(room) != first));
    }

    #[test]
    fn room_snapshots_start_empty_and_can_track_each_room() {
        let mut snapshots = RoomSnapshots::default();
        assert!(snapshots.enemies.is_empty());
        assert!(snapshots.projectiles.is_empty());
        snapshots.enemies.insert(IVec2::ZERO, Vec::new());
        snapshots.projectiles.insert(IVec2::new(1, 0), Vec::new());
        assert!(snapshots.enemies.contains_key(&IVec2::ZERO));
        assert!(snapshots.projectiles.contains_key(&IVec2::new(1, 0)));
    }

    #[test]
    fn room_population_keeps_minibosses_in_the_one_to_three_range() {
        let populations: Vec<_> = (-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| room_population(IVec2::new(x, y))))
            .collect();
        let miniboss_rooms = populations
            .iter()
            .filter(|population| population.minibosses > 0)
            .count();
        assert!((1..=3).contains(&miniboss_rooms));
    }

    #[test]
    fn room_population_keeps_thugs_in_the_four_to_seven_range() {
        let populations: Vec<_> = (-1..=1)
            .flat_map(|y| (-1..=1).map(move |x| room_population(IVec2::new(x, y))))
            .collect();
        let thug_rooms = populations
            .iter()
            .filter(|population| population.thugs > 0)
            .count();
        assert!((4..=7).contains(&thug_rooms));
    }

    #[test]
    fn miniboss_rooms_have_two_thugs_and_sparse_minions() {
        for y in -1..=1 {
            for x in -1..=1 {
                let population = room_population(IVec2::new(x, y));
                if population.minibosses > 0 {
                    assert_eq!(population.thugs, 2);
                    assert!(population.minions <= 1);
                }
            }
        }
    }
}
