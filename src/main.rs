//! Shapely: a top-down neon action game built with Bevy ECS.
//!
//! This binary owns shared run state and composes the focused gameplay modules
//! into the Bevy startup and update schedules.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use std::collections::HashMap;

mod aggressive;
mod audio;
mod damageable;
mod enemy;
mod items;
mod navigation;
mod player;
mod projectable;
mod room;
mod ui;
mod upgrades;
mod wall;

use items::ItemKind;
use navigation::RoomNavGrid;
use player::{BounceVelocity, Player};
use upgrades::UpgradeKind;

const PLAYER_SPEED: f32 = 280.0;
const BASE_HEALTH: i32 = 100;
const BASE_MANA: i32 = 100;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.09)))
        .insert_resource(GameState::default())
        .insert_resource(room::RoomSnapshots::default())
        .insert_resource(RoomNavGrid::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "shapely // neon rooms".into(),
                resolution: (1280.0_f32, 720.0_f32).into(),
                resizable: true,
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(projectable::plugin())
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_systems(Startup, (audio::setup_audio, setup).chain())
        .add_systems(
            Update,
            (
                ui::menu_input,
                ui::menu_button_interaction,
                ui::update_menu_visibility,
                restart_after_death,
                player::player_movement,
                wall::wall_collision,
                player::attack_input,
                damageable::animate_attacks,
                enemy::enemy_flash,
                enemy::enemy_ai,
                enemy::enemy_attacks,
                damageable::hostile_projectiles,
                damageable::enemy_player_collision,
                player::player_flash,
                player::collect_pickups,
                room::update_room,
                ui::toggle_inventory,
                player::use_inventory_items,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                upgrades::update_upgrade_overlay,
                upgrades::choose_upgrade,
                damageable::sync_player_damageable,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                ui::update_hud,
                ui::update_crosshair,
                ui::update_enemy_bars,
                ui::floating_text_system,
                aggressive::update_cooldowns,
                aggressive::lifetime_cleanup,
                projectable::animate_fx,
            )
                .chain(),
        )
        .run();
}

#[derive(Resource)]
struct GameState {
    mode: GameMode,
    room: IVec2,
    health: i32,
    mana: i32,
    max_health: i32,
    max_mana: i32,
    speed_boost: f32,
    damage_cooldown: f32,
    inventory_open: bool,
    inventory: Vec<ItemKind>,
    cooldowns: [f32; 3],
    attack_timer: f32,
    rooms: HashMap<IVec2, bool>,
    cooldown_multiplier: f32,
    attack_cooldown_multipliers: [f32; 3],
    attack_speed_boost: f32,
    projectile_scale: f32,
    knockback_multiplier: f32,
    move_speed_multiplier: f32,
    power_multiplier: f32,
    extra_attack: bool,
    upgrade_choices: [UpgradeKind; 3],
    chest_open_progress: f32,
    room_notice: f32,
}

impl Default for GameState {
    fn default() -> Self {
        let mut rooms = HashMap::new();
        for y in -1..=1 {
            for x in -1..=1 {
                rooms.insert(IVec2::new(x, y), false);
            }
        }
        Self {
            mode: GameMode::Title,
            room: IVec2::ZERO,
            health: BASE_HEALTH,
            mana: BASE_MANA,
            max_health: BASE_HEALTH,
            max_mana: BASE_MANA,
            speed_boost: 0.0,
            damage_cooldown: 0.0,
            inventory_open: false,
            inventory: Vec::new(),
            cooldowns: [0.0; 3],
            attack_timer: 0.0,
            rooms,
            cooldown_multiplier: 1.0,
            attack_cooldown_multipliers: [1.0; 3],
            attack_speed_boost: 0.0,
            projectile_scale: 1.0,
            knockback_multiplier: 1.0,
            move_speed_multiplier: 1.0,
            power_multiplier: 1.0,
            extra_attack: false,
            upgrade_choices: [
                UpgradeKind::Speed,
                UpgradeKind::AttackSpeed,
                UpgradeKind::Power,
            ],
            chest_open_progress: 0.0,
            room_notice: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameMode {
    Title,
    Playing,
    Paused,
    Settings,
    Upgrade,
    Dead,
}

#[derive(Component)]
struct Lifetime(pub(crate) f32);

#[derive(Component)]
struct RoomEntity;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nav: ResMut<RoomNavGrid>,
) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        Player {
            hit_flash: 0.0,
            health: BASE_HEALTH,
            max_health: BASE_HEALTH,
        },
        BounceVelocity(Vec2::ZERO),
        RigidBody::KinematicPositionBased,
        Collider::cuboid(18.0, 18.0),
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgb(1.0, 0.12, 0.4),
                custom_size: Some(Vec2::splat(38.0)),
                ..default()
            },
            transform: Transform::from_xyz(0.0, 0.0, 5.0),
            ..default()
        },
    ));
    room::build_room(
        &mut commands,
        &mut meshes,
        &mut materials,
        IVec2::ZERO,
        true,
        nav,
        asset_server.load("fonts/FiraSans-Bold.ttf"),
    );
    ui::spawn_hud(&mut commands, &asset_server);
}

fn restart_after_death(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut state: ResMut<GameState>,
    mut player: Query<&mut Transform, With<Player>>,
    old_room: Query<Entity, With<RoomEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    nav: ResMut<RoomNavGrid>,
    assets: Res<AssetServer>,
) {
    if state.mode != GameMode::Dead || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in &old_room {
        commands.entity(entity).despawn_recursive();
    }
    *state = GameState::default();
    state.mode = GameMode::Playing;
    if let Ok(mut transform) = player.get_single_mut() {
        transform.translation = Vec3::new(0.0, 0.0, 5.0);
    }
    room::build_room(
        &mut commands,
        &mut meshes,
        &mut materials,
        IVec2::ZERO,
        true,
        nav,
        assets.load("fonts/FiraSans-Bold.ttf"),
    );
}
