//! Shapely: a top-down neon action game built with Bevy ECS.
//!
//! This binary assembles the gameplay modules, owns the global run state, and
//! wires menus, rooms, combat, audio, and HUD systems into the Bevy schedule.

use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy_rapier2d::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::Arc;

mod damageable;
mod enemy;
mod navigation;
mod player;
mod projectable;
mod room;
mod wall;

use enemy::{Enemy, EnemyKind};
use damageable::Damageable;
use navigation::RoomNavGrid;
use player::{attack_hits_enemy, Attack, AttackKind, BounceVelocity, Player};
use projectable::{FxKind, FxMaterial, HostileProjectile};
use wall::{circle_hits_wall, spawn_wall, Wall};

use room::ROOM_SIZE;
const PLAYER_SPEED: f32 = 280.0;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.09)))
        .insert_resource(GameState::default())
        .insert_resource(RoomSnapshots::default())
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
        .add_systems(Startup, (setup_audio, setup).chain())
        .add_systems(Update, (
            menu_input,
            menu_button_interaction,
            update_menu_visibility,
            restart_after_death,
            player::player_movement,
            wall::wall_collision,
            player::attack_input,
            animate_attacks,
            enemy::enemy_flash,
            enemy::enemy_ai,
            enemy::enemy_attacks,
            hostile_projectiles,
            enemy_player_collision,
            player::player_flash,
            collect_pickups,
            update_room,
            toggle_inventory,
            use_inventory_items,
            choose_upgrade,
            sync_player_damageable,
        ).chain())
        .add_systems(Update, (update_hud, update_crosshair, update_enemy_bars, floating_text_system, update_cooldowns, lifetime_cleanup, projectable::animate_fx).chain())
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
    rooms: HashMap<IVec2, bool>,
    cooldown_multiplier: f32,
    projectile_scale: f32,
    knockback_multiplier: f32,
    room_notice: f32,
}

#[derive(Clone)]
struct EnemySnapshot {
    kind: EnemyKind,
    health: i32,
    position: Vec3,
    phase: f32,
    attack_timer: f32,
    summon_timer: f32,
}

#[derive(Clone)]
struct ProjectileSnapshot {
    kind: FxKind,
    position: Vec3,
    velocity: Vec2,
    damage: i32,
    radius: f32,
    lifetime: f32,
}

#[derive(Resource, Default)]
struct RoomSnapshots {
    enemies: HashMap<IVec2, Vec<EnemySnapshot>>,
    projectiles: HashMap<IVec2, Vec<ProjectileSnapshot>>,
}

impl Default for GameState {
    fn default() -> Self {
        let mut rooms = HashMap::new();
        for y in -1..=1 { for x in -1..=1 { rooms.insert(IVec2::new(x, y), false); } }
        Self { mode: GameMode::Title, room: IVec2::ZERO, health: 100, mana: 100, max_health: 100, max_mana: 100,
            speed_boost: 0.0, damage_cooldown: 0.0, inventory_open: false, inventory: Vec::new(), cooldowns: [0.0; 3], rooms,
            cooldown_multiplier: 1.0, projectile_scale: 1.0, knockback_multiplier: 1.0, room_notice: 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameMode { Title, Playing, Paused, Settings, Upgrade, Dead }

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind { HealthPotion, ManaPotion, SpeedPotion }
#[derive(Component)] struct Pickup { item: ItemKind }
#[derive(Component)] struct Lifetime(f32);
#[derive(Component)] struct Hud;
#[derive(Component)] struct HealthBarFill;
#[derive(Component)] struct InventoryPanel;
#[derive(Component)] struct InventoryText;
#[derive(Component)] struct RoomEntity;
#[derive(Component)] struct EnemyHealthBar { max_health: i32 }
#[derive(Component)] struct EnemyName;
#[derive(Component)] struct Crosshair;
#[derive(Component)] struct TitlePanel;
#[derive(Component)] struct PausePanel;
#[derive(Component)] struct SettingsPanel;
#[derive(Component)] struct UpgradePanel;
#[derive(Component)] struct DeathPanel;
#[derive(Component)] struct RoomClearText;
#[derive(Component)] struct FloatingText { velocity: Vec2 }
#[derive(Component)] struct MenuButton(MenuAction);
#[allow(dead_code)]
#[derive(Clone, Copy)] enum MenuAction { Play, Resume, Settings, Back, Restart, Quit }

#[derive(Clone, Copy)] enum SoundKind { Hit, Attack, Pickup, Transition }
#[derive(Resource, Clone)] struct AudioCues { hit: Handle<AudioSource>, attack: Handle<AudioSource>, pickup: Handle<AudioSource>, transition: Handle<AudioSource> }

fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let add_tone = |frequency: f32, duration: f32, volume: f32, sources: &mut Assets<AudioSource>| {
        sources.add(AudioSource { bytes: Arc::from(tone_wav(frequency, duration, volume)) })
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

fn play_sound(commands: &mut Commands, cues: Option<&AudioCues>, sound: SoundKind) {
    let Some(cues) = cues else { return };
    let source = match sound { SoundKind::Hit => &cues.hit, SoundKind::Attack => &cues.attack, SoundKind::Pickup => &cues.pickup, SoundKind::Transition => &cues.transition };
    commands.spawn(AudioBundle { source: source.clone(), settings: PlaybackSettings::DESPAWN });
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, nav: ResMut<RoomNavGrid>) {
    commands.spawn(Camera2dBundle::default());
    // Shapely is a top-down game: the screen plane is the floor. Keep Rapier
    // collision support, but never let gravity pull the player out of the room.
    commands.spawn((Player { hit_flash: 0.0, health: 100, max_health: 100 }, BounceVelocity(Vec2::ZERO), RigidBody::KinematicPositionBased, Collider::cuboid(18.0, 18.0),
        SpriteBundle { sprite: Sprite { color: Color::srgb(1.0, 0.12, 0.4), custom_size: Some(Vec2::splat(38.0)), ..default() }, transform: Transform::from_xyz(0.0, 0.0, 5.0), ..default() }));
    build_room(&mut commands, &mut meshes, &mut materials, IVec2::ZERO, true, nav, asset_server.load("fonts/FiraSans-Bold.ttf"));
    spawn_hud(&mut commands, &asset_server);
}

fn spawn_hud(commands: &mut Commands, assets: &Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((Hud, TextBundle { text: Text::from_section("", TextStyle { font: font.clone(), font_size: 18.0, color: Color::WHITE }), style: Style { position_type: PositionType::Absolute, left: Val::Px(24.0), top: Val::Px(18.0), ..default() }, ..default() }));
    commands.spawn(NodeBundle { style: Style { position_type: PositionType::Absolute, left: Val::Px(24.0), top: Val::Px(72.0), width: Val::Px(220.0), height: Val::Px(18.0), ..default() }, background_color: BackgroundColor(Color::srgba(0.12, 0.02, 0.05, 0.95)), ..default() }).with_children(|parent| {
        parent.spawn((HealthBarFill, NodeBundle { style: Style { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, background_color: BackgroundColor(Color::srgb(1.0, 0.12, 0.25)), ..default() }));
    });
    commands.spawn((InventoryPanel, NodeBundle { style: Style { display: Display::None, position_type: PositionType::Absolute, left: Val::Percent(18.0), top: Val::Percent(12.0), width: Val::Percent(64.0), height: Val::Percent(70.0), padding: UiRect::all(Val::Px(28.0)), flex_direction: FlexDirection::Column, ..default() }, background_color: BackgroundColor(Color::srgba(0.04, 0.055, 0.14, 0.97)), ..default() })).with_children(|parent| {
        parent.spawn((InventoryText, TextBundle { text: Text::from_section("INVENTORY // 24 SLOTS", TextStyle { font: font.clone(), font_size: 24.0, color: Color::srgb(0.3, 0.9, 1.0) }), ..default() }));
    });
    commands.spawn((Crosshair, TextBundle { text: Text::from_section("✦", TextStyle { font: assets.load("fonts/FiraSans-Bold.ttf"), font_size: 22.0, color: Color::srgb(0.3, 0.95, 1.0) }), style: Style { position_type: PositionType::Absolute, ..default() }, ..default() }));
    commands.spawn((RoomClearText, TextBundle { text: Text::from_section("", TextStyle { font: assets.load("fonts/FiraSans-Bold.ttf"), font_size: 22.0, color: Color::srgb(0.4, 1.0, 0.65) }), style: Style { position_type: PositionType::Absolute, left: Val::Percent(38.0), top: Val::Px(28.0), ..default() }, ..default() }));
    spawn_controls_overlay(commands, &font, TitlePanel, true);
    spawn_controls_overlay(commands, &font, PausePanel, false);
    spawn_overlay(commands, &font, SettingsPanel, "SETTINGS\n\nAudio cues: ON\nDisplay: neon\n\nPress Esc to return", false);
    spawn_overlay(commands, &font, UpgradePanel, "ROOM CLEARED\n\nCHOOSE A PERMANENT UPGRADE\n\n[1] +20 MAX HEALTH\n[2] FASTER COOLDOWNS\n[3] LARGER PROJECTILES\n[4] +30 MAX MANA\n[5] MORE KNOCKBACK", false);
    spawn_overlay(commands, &font, DeathPanel, "YOU WERE SHATTERED\n\nPress R to RESTART", false);
}

fn spawn_overlay<T: Component>(commands: &mut Commands, font: &Handle<Font>, marker: T, message: &str, title: bool) {
    let panel = commands.spawn((marker, NodeBundle { style: Style { display: if title { Display::Flex } else { Display::None }, position_type: PositionType::Absolute, left: Val::Percent(17.0), top: Val::Percent(12.0), width: Val::Percent(66.0), height: Val::Percent(76.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, background_color: BackgroundColor(Color::srgba(0.025, 0.035, 0.12, 0.97)), ..default() })).id();
    commands.entity(panel).with_children(|parent| { parent.spawn(TextBundle { text: Text::from_section(message, TextStyle { font: font.clone(), font_size: if title { 27.0 } else { 22.0 }, color: Color::srgb(0.55, 0.95, 1.0) }), ..default() }); });
}

fn spawn_controls_overlay<T: Component>(commands: &mut Commands, font: &Handle<Font>, marker: T, title: bool) {
    let panel = commands.spawn((marker, NodeBundle {
        style: Style {
            display: if title { Display::Flex } else { Display::None },
            position_type: PositionType::Absolute,
            left: Val::Percent(17.0),
            top: Val::Percent(12.0),
            width: Val::Percent(66.0),
            height: Val::Percent(76.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        background_color: BackgroundColor(Color::srgba(0.025, 0.035, 0.12, 0.97)),
        ..default()
    })).id();
    commands.entity(panel).with_children(|parent| {
        parent.spawn(NodeBundle {
            style: Style { width: Val::Px(560.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
            ..default()
        }).with_children(|content| {
            content.spawn(TextBundle { text: Text::from_section(if title { "SHAPELY" } else { "PAUSED" }, TextStyle { font: font.clone(), font_size: 34.0, color: Color::srgb(0.3, 0.95, 1.0) }), ..default() });
            content.spawn(TextBundle { text: Text::from_section(if title { "PLAY" } else { "RESUME" }, TextStyle { font: font.clone(), font_size: 27.0, color: Color::srgb(0.3, 0.95, 1.0) }), style: Style { margin: UiRect::bottom(Val::Px(28.0)), ..default() }, ..default() });
            content.spawn(TextBundle { text: Text::from_section("ATTACK CONTROLS", TextStyle { font: font.clone(), font_size: 21.0, color: Color::srgb(1.0, 0.45, 0.75) }), style: Style { margin: UiRect::bottom(Val::Px(10.0)), ..default() }, ..default() });
            content.spawn(NodeBundle { style: Style { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, margin: UiRect::bottom(Val::Px(5.0)), ..default() }, ..default() }).with_children(|row| {
                row.spawn(TextBundle { text: Text::from_section("ATTACK", TextStyle { font: font.clone(), font_size: 17.0, color: Color::srgb(0.55, 0.95, 1.0) }), style: Style { width: Val::Px(190.0), ..default() }, ..default() });
                row.spawn(TextBundle { text: Text::from_section("KEYBOARD", TextStyle { font: font.clone(), font_size: 17.0, color: Color::srgb(0.55, 0.95, 1.0) }), style: Style { width: Val::Px(180.0), ..default() }, ..default() });
                row.spawn(TextBundle { text: Text::from_section("MOUSE", TextStyle { font: font.clone(), font_size: 17.0, color: Color::srgb(0.55, 0.95, 1.0) }), ..default() });
            });
            for (attack, keyboard, mouse) in [("LASERBEAM", "1", "G5"), ("SLASH", "2", "Right-click"), ("FIREBALL", "3", "G4")] {
                content.spawn(NodeBundle { style: Style { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, margin: UiRect::bottom(Val::Px(7.0)), ..default() }, ..default() }).with_children(|row| {
                    row.spawn(TextBundle { text: Text::from_section(attack, TextStyle { font: font.clone(), font_size: 20.0, color: Color::WHITE }), style: Style { width: Val::Px(190.0), ..default() }, ..default() });
                    row.spawn(TextBundle { text: Text::from_section(keyboard, TextStyle { font: font.clone(), font_size: 20.0, color: Color::WHITE }), style: Style { width: Val::Px(180.0), ..default() }, ..default() });
                    row.spawn(TextBundle { text: Text::from_section(mouse, TextStyle { font: font.clone(), font_size: 20.0, color: Color::WHITE }), ..default() });
                });
            }
            content.spawn(TextBundle { text: Text::from_section("WASD / ARROWS   move\nI   inventory\n? / Esc   pause", TextStyle { font: font.clone(), font_size: 20.0, color: Color::srgb(0.55, 0.95, 1.0) }), style: Style { margin: UiRect::top(Val::Px(22.0)), ..default() }, ..default() });
            content.spawn(TextBundle { text: Text::from_section(if title { "Press ENTER to PLAY" } else { "Press ? or Esc to resume\nQ   quit" }, TextStyle { font: font.clone(), font_size: 20.0, color: Color::srgb(0.55, 0.95, 1.0) }), style: Style { margin: UiRect::top(Val::Px(24.0)), ..default() }, ..default() });
        });
    });
}

fn room_seed(room: IVec2) -> u32 {
    room::seed(room)
}

fn room_walls(room: IVec2) -> Vec<(Vec2, Vec2)> {
    let seed = room_seed(room);
    let mut walls = Vec::new();
    if room.y < 1 { walls.push((Vec2::new(-350.0, 305.0), Vec2::new(410.0, 12.0))); walls.push((Vec2::new(350.0, 305.0), Vec2::new(410.0, 12.0))); } else { walls.push((Vec2::new(0.0, 305.0), Vec2::new(1120.0, 12.0))); }
    if room.y > -1 { walls.push((Vec2::new(-350.0, -305.0), Vec2::new(410.0, 12.0))); walls.push((Vec2::new(350.0, -305.0), Vec2::new(410.0, 12.0))); } else { walls.push((Vec2::new(0.0, -305.0), Vec2::new(1120.0, 12.0))); }
    if room.x > -1 { walls.push((Vec2::new(-555.0, -150.0), Vec2::new(12.0, 290.0))); walls.push((Vec2::new(-555.0, 225.0), Vec2::new(12.0, 140.0))); } else { walls.push((Vec2::new(-555.0, 0.0), Vec2::new(12.0, 620.0))); }
    if room.x < 1 { walls.push((Vec2::new(555.0, -150.0), Vec2::new(12.0, 290.0))); walls.push((Vec2::new(555.0, 225.0), Vec2::new(12.0, 140.0))); } else { walls.push((Vec2::new(555.0, 0.0), Vec2::new(12.0, 620.0))); }
    let wall_count = match seed % 4 { 0 => 1, 1 => 3, 2 => 5, _ => 7 };
    for i in 0..wall_count { let value = seed.wrapping_add(i * 0x45D9_F3B); let x = -360.0 + ((value % 7) as f32) * 120.0; let y = -190.0 + (((value / 7) % 5) as f32) * 95.0; let horizontal_wall = value & 1 == 0; let size = if horizontal_wall { Vec2::new(150.0 + ((value / 13) % 2) as f32 * 70.0, 12.0) } else { Vec2::new(12.0, 115.0 + ((value / 17) % 2) as f32 * 55.0) }; walls.push((Vec2::new(x, y), size)); }
    walls
}

fn build_room(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<ColorMaterial>>, room: IVec2, populated: bool, mut nav: ResMut<RoomNavGrid>, font: Handle<Font>) {
    let seed = room_seed(room);
    commands.spawn((RoomEntity, SpriteBundle { sprite: Sprite { color: Color::srgb(0.035 + (seed % 4) as f32 * 0.006, 0.045, 0.12 + (seed % 3) as f32 * 0.008), custom_size: Some(ROOM_SIZE), ..default() }, transform: Transform::from_xyz(0.0, 0.0, -2.0), ..default() }));
    let wall_color = Color::srgb(0.18, 0.22, 0.5);
    let walls = room_walls(room);
    for (position, size) in &walls { spawn_wall(commands, *position, *size, if size.x > size.y { wall_color } else { Color::srgb(0.12, 0.28, 0.48) }); }
    nav.rebuild(room, &walls);
    let accents = [(-380.0, 190.0), (360.0, 155.0), (-280.0, -190.0), (260.0, -190.0)];
    for (index, (x, y)) in accents.iter().enumerate() { if (seed + index as u32) % 3 != 0 { commands.spawn((RoomEntity, MaterialMesh2dBundle { mesh: meshes.add(Rectangle::new(90.0, 5.0)).into(), material: materials.add(ColorMaterial::from(Color::srgb(0.06, 0.8, 0.92))), transform: Transform::from_xyz(*x, *y, -1.0), ..default() })); } }
    if populated { spawn_enemies(commands, meshes, materials, room, font); }
}

fn spawn_enemies(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<ColorMaterial>>, room: IVec2, font: Handle<Font>) {
    let shift = (room_seed(room) % 5) as f32 * 18.0;
    let specs = [(EnemyKind::Minion, Vec2::new(-260.0 + shift, 120.0)), (EnemyKind::Minion, Vec2::new(250.0 - shift, -80.0)), (EnemyKind::Thug, Vec2::new(180.0, 180.0 - shift)), (EnemyKind::Miniboss, Vec2::new(0.0, -160.0 + shift))];
    for (kind, pos) in specs {
        spawn_enemy(commands, meshes, materials, kind, pos, room, font.clone());
    }
}

fn spawn_enemy(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<ColorMaterial>>, kind: EnemyKind, pos: Vec2, room: IVec2, font: Handle<Font>) -> Entity {
    let (hp, radius, _) = enemy::spec(kind);
    let (mesh, color) = match kind { EnemyKind::Minion => (meshes.add(Circle::new(radius)), Color::srgb(0.2, 0.9, 0.72)), EnemyKind::Thug => (meshes.add(RegularPolygon::new(radius, 6)), Color::srgb(1.0, 0.48, 0.16)), EnemyKind::Miniboss => (meshes.add(RegularPolygon::new(radius, 8)), Color::srgb(0.78, 0.25, 1.0)) };
    let entity = commands.spawn((RoomEntity, BounceVelocity(Vec2::ZERO), Enemy { kind, health: hp, room, radius, phase: pos.x * 0.01, path: Vec::new(), path_index: 0, planned_goal: None, repath_timer: 0.0, hit_flash: 0.0, base_color: color, attack_timer: 1.5, summon_timer: 4.0 }, MaterialMesh2dBundle { mesh: mesh.into(), material: materials.add(ColorMaterial::from(color)), transform: Transform::from_xyz(pos.x, pos.y, 3.0), ..default() })).id();
    commands.entity(entity).with_children(|parent| {
        parent.spawn((EnemyHealthBar { max_health: hp }, SpriteBundle { sprite: Sprite { color: Color::srgb(0.2, 0.95, 0.45), custom_size: Some(Vec2::new(radius * 2.0, 5.0)), ..default() }, transform: Transform::from_xyz(0.0, radius + 10.0, 1.0), ..default() }));
        if matches!(kind, EnemyKind::Miniboss) { parent.spawn((EnemyName, Text2dBundle { text: Text::from_section("MINIBOSS", TextStyle { font, font_size: 15.0, color: Color::srgb(1.0, 0.55, 0.85) }), transform: Transform::from_xyz(0.0, radius + 21.0, 1.0), ..default() })); }
    });
    entity
}

fn restore_room_snapshot(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, color_materials: &mut ResMut<Assets<ColorMaterial>>, fx_materials: &mut ResMut<Assets<FxMaterial>>, snapshots: &RoomSnapshots, room: IVec2, font: Handle<Font>) {
    if let Some(enemies) = snapshots.enemies.get(&room) {
        for snapshot in enemies {
            let entity = spawn_enemy(commands, meshes, color_materials, snapshot.kind, snapshot.position.truncate(), room, font.clone());
            let (_, radius, _) = enemy::spec(snapshot.kind);
            commands.entity(entity).insert(Enemy {
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
                base_color: match snapshot.kind { EnemyKind::Minion => Color::srgb(0.2, 0.9, 0.72), EnemyKind::Thug => Color::srgb(1.0, 0.48, 0.16), EnemyKind::Miniboss => Color::srgb(0.78, 0.25, 1.0) },
                attack_timer: snapshot.attack_timer,
                summon_timer: snapshot.summon_timer,
            });
        }
    }
    if let Some(projectiles) = snapshots.projectiles.get(&room) {
        for snapshot in projectiles {
            let direction = snapshot.velocity.normalize_or_zero();
            let speed = snapshot.velocity.length();
            let entity = projectable::spawn_hostile_projectile(commands, meshes, fx_materials, snapshot.kind, snapshot.position.truncate(), if direction == Vec2::ZERO { Vec2::X } else { direction }, speed, snapshot.damage, snapshot.radius);
            commands.entity(entity).insert((Transform::from_translation(snapshot.position), HostileProjectile { velocity: snapshot.velocity, damage: snapshot.damage, radius: snapshot.radius, kind: snapshot.kind }, Lifetime(snapshot.lifetime)));
        }
    }
}

fn question_mark_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.just_pressed(KeyCode::Slash) && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
}

fn menu_input(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>, mut exit: EventWriter<AppExit>) {
    let question_mark = question_mark_pressed(&keys);
    match state.mode {
        GameMode::Title => if keys.just_pressed(KeyCode::Enter) { state.mode = GameMode::Playing },
        GameMode::Playing => if question_mark || (keys.just_pressed(KeyCode::Escape) && !state.inventory_open) { state.mode = GameMode::Paused },
        GameMode::Paused => if question_mark || keys.just_pressed(KeyCode::Escape) { state.mode = GameMode::Playing },
        GameMode::Settings => if keys.just_pressed(KeyCode::Escape) { state.mode = GameMode::Title },
        GameMode::Upgrade | GameMode::Dead => {}
    }
    if keys.just_pressed(KeyCode::KeyQ) && matches!(state.mode, GameMode::Title | GameMode::Paused) { exit.send(AppExit::Success); }
}

fn menu_button_interaction(mut state: ResMut<GameState>, buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed { continue; }
        state.mode = match button.0 { MenuAction::Play | MenuAction::Resume => GameMode::Playing, MenuAction::Settings => GameMode::Settings, MenuAction::Back => GameMode::Title, MenuAction::Restart => GameMode::Dead, MenuAction::Quit => state.mode };
    }
}

fn update_menu_visibility(state: Res<GameState>, mut panels: Query<(&mut Style, Option<&TitlePanel>, Option<&PausePanel>, Option<&SettingsPanel>, Option<&UpgradePanel>, Option<&DeathPanel>), Or<(With<TitlePanel>, With<PausePanel>, With<SettingsPanel>, With<UpgradePanel>, With<DeathPanel>)>>) {
    for (mut style, title, pause, settings, upgrade, death) in &mut panels {
        let visible = (title.is_some() && state.mode == GameMode::Title) || (pause.is_some() && state.mode == GameMode::Paused) || (settings.is_some() && state.mode == GameMode::Settings) || (upgrade.is_some() && state.mode == GameMode::Upgrade) || (death.is_some() && state.mode == GameMode::Dead);
        style.display = if visible { Display::Flex } else { Display::None };
    }
}

fn restart_after_death(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands, mut state: ResMut<GameState>, mut player: Query<&mut Transform, With<Player>>, old_room: Query<Entity, With<RoomEntity>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, nav: ResMut<RoomNavGrid>, assets: Res<AssetServer>) {
    if state.mode != GameMode::Dead || !keys.just_pressed(KeyCode::KeyR) { return; }
    for entity in &old_room { commands.entity(entity).despawn_recursive(); }
    *state = GameState::default();
    state.mode = GameMode::Playing;
    if let Ok(mut transform) = player.get_single_mut() { transform.translation = Vec3::new(0.0, 0.0, 5.0); }
    build_room(&mut commands, &mut meshes, &mut materials, IVec2::ZERO, true, nav, assets.load("fonts/FiraSans-Bold.ttf"));
}

fn animate_attacks(time: Res<Time>, mut attacks: Query<(Entity, &mut Transform, &mut Attack), Without<Enemy>>, mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Attack>>, walls: Query<(&Transform, &Wall), (Without<Attack>, Without<Enemy>)>, mut commands: Commands, mut state: ResMut<GameState>, mut meshes: Option<ResMut<Assets<Mesh>>>, mut fx_materials: Option<ResMut<Assets<FxMaterial>>>, cues: Option<Res<AudioCues>>) {
    if state.mode != GameMode::Playing { return; }
    let mut killed_rooms = Vec::new();
    for (attack_entity, mut transform, mut attack) in &mut attacks {
        let next_position = transform.translation + attack.velocity.extend(0.0) * time.delta_seconds();
        if attack.kind != AttackKind::Slash {
            let radius = if attack.kind == AttackKind::Laser { 6.0 } else { 18.0 };
            let hits_wall = walls.iter().any(|(wall_transform, wall)| circle_hits_wall(next_position.truncate(), radius, wall_transform.translation.truncate(), wall.size));
            if hits_wall { commands.entity(attack_entity).despawn_recursive(); continue; }
        }
        transform.translation = next_position;
        attack.age += time.delta_seconds();
        if attack.kind == AttackKind::Slash { transform.rotation = Quat::from_rotation_z(attack.age * 12.0); transform.scale = Vec3::splat(1.0 + attack.age * 2.0); }
        let mut hit_enemy = false;
        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies { let can_hit = (attack.kind != AttackKind::Slash || attack.age < 0.12 + time.delta_seconds()) && !attack.hit_targets.contains(&enemy_entity) && attack_hits_enemy(&transform, attack.kind, enemy_transform.translation.truncate(), enemy.radius); if can_hit { attack.hit_targets.push(enemy_entity); enemy.take_damage(attack.damage); enemy.hit_flash = 0.225; if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut()) { projectable::spawn_fx(&mut commands, meshes, fx_materials, FxKind::EnemyDamage, enemy_transform.translation + Vec3::new(0.0, 0.0, 8.0), Vec2::splat(enemy.radius * 4.2), 0.38); } if !enemy.is_alive() { let drop = match enemy.kind { EnemyKind::Minion => ItemKind::ManaPotion, EnemyKind::Thug => ItemKind::HealthPotion, EnemyKind::Miniboss => ItemKind::SpeedPotion }; killed_rooms.push(enemy.room); commands.spawn((RoomEntity, Pickup { item: drop }, SpriteBundle { sprite: Sprite { color: item_color(drop), custom_size: Some(Vec2::splat(22.0)), ..default() }, transform: *enemy_transform, ..default() })); commands.entity(enemy_entity).despawn_recursive(); } if attack.kind != AttackKind::Slash { hit_enemy = true; } break; } }
        if hit_enemy { play_sound(&mut commands, cues.as_deref(), SoundKind::Hit); commands.entity(attack_entity).despawn_recursive(); }
    }
    killed_rooms.sort_by_key(|room| (room.x, room.y));
    killed_rooms.dedup();
    for room in killed_rooms { if !enemies.iter().any(|(_, enemy, _)| enemy.room == room && enemy.health > 0) { state.rooms.insert(room, true); state.room_notice = 3.0; state.mode = GameMode::Upgrade; } }
}

fn item_color(item: ItemKind) -> Color { match item { ItemKind::HealthPotion => Color::srgb(1.0, 0.15, 0.25), ItemKind::ManaPotion => Color::srgb(0.2, 0.55, 1.0), ItemKind::SpeedPotion => Color::srgb(0.9, 0.2, 1.0) } }

fn hostile_projectiles(time: Res<Time>, mut commands: Commands, mut state: ResMut<GameState>, mut projectiles: Query<(Entity, &mut Transform, &HostileProjectile), Without<Player>>, walls: Query<(&Transform, &Wall), (Without<HostileProjectile>, Without<Player>)>, mut player: Query<(&mut Transform, &mut BounceVelocity, &mut Player)>, mut meshes: Option<ResMut<Assets<Mesh>>>, mut fx_materials: Option<ResMut<Assets<FxMaterial>>>, cues: Option<Res<AudioCues>>) {
    if state.mode != GameMode::Playing { return; }
    let Ok((player_transform, mut player_bounce, mut player_data)) = player.get_single_mut() else { return };
    for (entity, mut transform, projectile) in &mut projectiles {
        let next = transform.translation + projectile.velocity.extend(0.0) * time.delta_seconds();
        if walls.iter().any(|(wall_transform, wall)| circle_hits_wall(next.truncate(), projectile.radius, wall_transform.translation.truncate(), wall.size)) { if let Some(effect_kind) = projectable::hostile_impact_effect(projectile.kind) { if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut()) { projectable::spawn_fx(&mut commands, meshes, fx_materials, effect_kind, next.with_z(6.0), Vec2::splat(projectile.radius * 3.4), 0.35); } } commands.entity(entity).despawn_recursive(); continue; }
        transform.translation = next;
        if transform.translation.truncate().distance(player_transform.translation.truncate()) < projectile.radius + 19.0 {
            let normal = (player_transform.translation - transform.translation).truncate().normalize_or_zero();
            player_bounce.0 = normal * 190.0 * state.knockback_multiplier;
            player_data.hit_flash = 0.225;
            if state.damage_cooldown <= 0.0 { player_data.take_damage(projectile.damage); state.health = player_data.health; state.damage_cooldown = 0.55; if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut()) { projectable::spawn_fx(&mut commands, meshes, fx_materials, FxKind::PlayerDamage, player_transform.translation + Vec3::new(0.0, 0.0, 8.0), Vec2::splat(88.0), 0.46); } if !player_data.is_alive() { state.mode = GameMode::Dead; } }
            if let Some(effect_kind) = projectable::hostile_impact_effect(projectile.kind) { if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut()) { projectable::spawn_fx(&mut commands, meshes, fx_materials, effect_kind, transform.translation.with_z(6.0), Vec2::splat(projectile.radius * 3.4), 0.35); } }
            play_sound(&mut commands, cues.as_deref(), SoundKind::Hit);
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn choose_upgrade(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>) {
    if state.mode != GameMode::Upgrade { return; }
    let choice = if keys.just_pressed(KeyCode::Digit1) { Some(1) } else if keys.just_pressed(KeyCode::Digit2) { Some(2) } else if keys.just_pressed(KeyCode::Digit3) { Some(3) } else if keys.just_pressed(KeyCode::Digit4) { Some(4) } else if keys.just_pressed(KeyCode::Digit5) { Some(5) } else { None };
    let Some(choice) = choice else { return };
    match choice { 1 => { state.max_health += 20; state.health = state.max_health; }, 2 => state.cooldown_multiplier *= 0.85, 3 => state.projectile_scale *= 1.18, 4 => { state.max_mana += 30; state.mana = state.max_mana; }, 5 => state.knockback_multiplier *= 1.25, _ => {} }
    state.mode = GameMode::Playing;
}

fn sync_player_damageable(state: Res<GameState>, mut player: Query<&mut Player>) {
    let Ok(mut player) = player.get_single_mut() else { return };
    player.max_health = state.max_health;
    player.health = state.health;
}

fn enemy_player_collision(mut player: Query<(&mut Transform, &mut BounceVelocity, &mut Player)>, mut enemies: Query<(&mut Transform, &mut BounceVelocity, &Enemy), Without<Player>>, mut state: ResMut<GameState>, mut commands: Commands, mut meshes: Option<ResMut<Assets<Mesh>>>, mut fx_materials: Option<ResMut<Assets<FxMaterial>>>) {
    if state.mode != GameMode::Playing { return; }
    let Ok((mut player, mut player_bounce, mut player_data)) = player.get_single_mut() else { return };
    for (mut enemy, mut enemy_bounce, enemy_data) in &mut enemies {
        let delta = player.translation.truncate() - enemy.translation.truncate();
        let distance = delta.length();
        let minimum_distance = 19.0 + enemy_data.radius;
        if distance < minimum_distance {
            let normal = if distance > 0.001 { delta / distance } else { Vec2::Y };
            let overlap = minimum_distance - distance + 2.0;
            // Equal and opposite impulse: the player gets knocked away from
            // the enemy, while the enemy recoils in the other direction.
            player.translation += (normal * overlap * 0.5).extend(0.0);
            enemy.translation -= (normal * overlap * 0.5).extend(0.0);
            player_bounce.0 = normal * 260.0 * state.knockback_multiplier;
            enemy_bounce.0 = -normal * 220.0 * state.knockback_multiplier;
            player_data.hit_flash = 0.225;
            if state.damage_cooldown <= 0.0 {
                let contact_damage = match enemy_data.kind { EnemyKind::Minion => enemy::minion::CONTACT_DAMAGE, EnemyKind::Thug => enemy::thug::CONTACT_DAMAGE, EnemyKind::Miniboss => enemy::miniboss::CONTACT_DAMAGE };
                player_data.take_damage(contact_damage);
                state.health = player_data.health;
                state.damage_cooldown = 0.55;
                if let (Some(meshes), Some(fx_materials)) = (meshes.as_mut(), fx_materials.as_mut()) { projectable::spawn_fx(&mut commands, meshes, fx_materials, FxKind::PlayerDamage, player.translation + Vec3::new(0.0, 0.0, 8.0), Vec2::splat(88.0), 0.46); }
                if !player_data.is_alive() { state.mode = GameMode::Dead; }
            }
        }
    }
}

fn collect_pickups(mut commands: Commands, mut state: ResMut<GameState>, player: Query<&Transform, (With<Player>, Without<Pickup>)>, pickups: Query<(Entity, &Pickup, &Transform), Without<Player>>, assets: Option<Res<AssetServer>>, cues: Option<Res<AudioCues>>) {
    if state.mode != GameMode::Playing { return; }
    let Ok(player) = player.get_single() else { return };
    for (entity, pickup, transform) in &pickups {
        if player.translation.distance(transform.translation) < 38.0 && state.inventory.len() < 24 {
            state.inventory.push(pickup.item);
            let message = match pickup.item { ItemKind::HealthPotion => "+30 HP", ItemKind::ManaPotion => "+35 MANA", ItemKind::SpeedPotion => "SPEED BOOST" };
            if let Some(assets) = assets.as_ref() {
                commands.spawn((FloatingText { velocity: Vec2::new(0.0, 26.0) }, Lifetime(1.2), Text2dBundle { text: Text::from_section(message, TextStyle { font: assets.load("fonts/FiraSans-Bold.ttf"), font_size: 18.0, color: item_color(pickup.item) }), transform: Transform::from_translation(transform.translation + Vec3::new(0.0, 20.0, 8.0)), ..default() }));
            }
            play_sound(&mut commands, cues.as_deref(), SoundKind::Pickup);
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn update_room(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<GameState>,
    mut snapshots: ResMut<RoomSnapshots>,
    mut player: Query<&mut Transform, With<Player>>,
    enemies: Query<(&Enemy, &Transform), (With<Enemy>, Without<Player>, Without<HostileProjectile>)>,
    projectiles: Query<(&HostileProjectile, &Transform, &Lifetime), (With<HostileProjectile>, Without<Player>, Without<Enemy>)>,
    old_room: Query<Entity, With<RoomEntity>>,
    projectile_entities: Query<Entity, With<HostileProjectile>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut fx_materials: ResMut<Assets<FxMaterial>>,
    nav: ResMut<RoomNavGrid>,
    assets: Res<AssetServer>,
    cues: Option<Res<AudioCues>>,
) {
    if state.mode != GameMode::Playing { return; }
    let Ok(mut player) = player.get_single_mut() else { return };
    let mut next = state.room;
    if player.translation.x < -548.0 { next.x -= 1; player.translation.x = 520.0; }
    if player.translation.x > 548.0 { next.x += 1; player.translation.x = -520.0; }
    if player.translation.y < -298.0 { next.y -= 1; player.translation.y = 270.0; }
    if player.translation.y > 298.0 { next.y += 1; player.translation.y = -270.0; }
    if next.x.abs() > 1 || next.y.abs() > 1 {
        player.translation.x = player.translation.x.clamp(-525.0, 525.0);
        player.translation.y = player.translation.y.clamp(-275.0, 275.0);
        return;
    }
    if next != state.room {
        let current_room = state.room;
        snapshots.enemies.insert(current_room, enemies.iter().map(|(enemy, transform)| EnemySnapshot { kind: enemy.kind, health: enemy.health, position: transform.translation, phase: enemy.phase, attack_timer: enemy.attack_timer, summon_timer: enemy.summon_timer }).collect());
        snapshots.projectiles.insert(current_room, projectiles.iter().map(|(projectile, transform, lifetime)| ProjectileSnapshot { kind: projectile.kind, position: transform.translation, velocity: projectile.velocity, damage: projectile.damage, radius: projectile.radius, lifetime: lifetime.0 }).collect());
        for entity in &old_room { commands.entity(entity).despawn_recursive(); }
        for entity in &projectile_entities { commands.entity(entity).despawn_recursive(); }
        state.room = next;
        let has_snapshot = snapshots.enemies.contains_key(&next);
        let populated = !*state.rooms.get(&next).unwrap_or(&false) && !has_snapshot;
        build_room(&mut commands, &mut meshes, &mut color_materials, next, populated, nav, assets.load("fonts/FiraSans-Bold.ttf"));
        restore_room_snapshot(&mut commands, &mut meshes, &mut color_materials, &mut fx_materials, &snapshots, next, assets.load("fonts/FiraSans-Bold.ttf"));
        play_sound(&mut commands, cues.as_deref(), SoundKind::Transition);
    }
    let _ = keys;
}

fn toggle_inventory(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>, mut panel: Query<&mut Style, With<InventoryPanel>>) { if state.mode != GameMode::Playing { return; } if keys.just_pressed(KeyCode::KeyI) || keys.just_pressed(KeyCode::Escape) { state.inventory_open = if keys.just_pressed(KeyCode::Escape) { false } else { !state.inventory_open }; if let Ok(mut style) = panel.get_single_mut() { style.display = if state.inventory_open { Display::Flex } else { Display::None }; } } }

fn use_inventory_items(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>) {
    if state.mode != GameMode::Playing { return; }
    let requested = if keys.just_pressed(KeyCode::KeyH) { Some(ItemKind::HealthPotion) } else if keys.just_pressed(KeyCode::KeyM) { Some(ItemKind::ManaPotion) } else if keys.just_pressed(KeyCode::KeyB) { Some(ItemKind::SpeedPotion) } else { None };
    let Some(item) = requested else { return };
    consume_item(&mut state, item);
}

fn consume_item(state: &mut GameState, item: ItemKind) -> bool {
    let Some(index) = state.inventory.iter().position(|stored| *stored == item) else { return false };
    state.inventory.remove(index);
    match item { ItemKind::HealthPotion => state.health = (state.health + 30).min(state.max_health), ItemKind::ManaPotion => state.mana = (state.mana + 35).min(state.max_mana), ItemKind::SpeedPotion => state.speed_boost = 25.0 }
    true
}

fn update_hud(state: Res<GameState>, mut hud: Query<&mut Text, (With<Hud>, Without<InventoryText>, Without<RoomClearText>)>, mut inventory_text: Query<&mut Text, (With<InventoryText>, Without<Hud>, Without<RoomClearText>)>, mut health_bar: Query<&mut Style, With<HealthBarFill>>, mut clear_text: Query<&mut Text, With<RoomClearText>>) { let inv = state.inventory.iter().take(6).map(|i| match i { ItemKind::HealthPotion => "♥", ItemKind::ManaPotion => "◆", ItemKind::SpeedPotion => "✦" }).collect::<Vec<_>>().join(" "); if let Ok(mut text) = hud.get_single_mut() { text.sections[0].value = format!("SHAPELY  //  ROOM {:+},{:+}\nHP {:>3}/{}   MANA {:>3}/{}   [1] LASER  [2] SLASH  [3] FIREBALL\nQUICK SLOTS: {}   [I] INVENTORY   [H] HEALTH  [M] MANA  [B] BOOST", state.room.x, state.room.y, state.health, state.max_health, state.mana, state.max_mana, if inv.is_empty() { "—".into() } else { inv }); } if let Ok(mut text) = inventory_text.get_single_mut() { let slots = (0..24).map(|slot| match state.inventory.get(slot) { Some(ItemKind::HealthPotion) => "[♥ HP]", Some(ItemKind::ManaPotion) => "[◆ MP]", Some(ItemKind::SpeedPotion) => "[✦ SPD]", None => "[     ]" }).collect::<Vec<_>>().join("  "); text.sections[0].value = format!("INVENTORY // 24 SLOTS\n\n{}\n\nH health   M mana   B speed (25 seconds)\nI / Esc close", slots); } if let Ok(mut style) = health_bar.get_single_mut() { style.width = Val::Percent((state.health.max(0) as f32 / state.max_health as f32) * 100.0); } if let Ok(mut text) = clear_text.get_single_mut() { text.sections[0].value = if state.room_notice > 0.0 { "ROOM CLEARED  //  CHOOSE AN UPGRADE".into() } else { "".into() }; } }

fn update_crosshair(state: Res<GameState>, windows: Query<&Window>, mut crosshair: Query<&mut Style, With<Crosshair>>) { let Ok(window) = windows.get_single() else { return }; let Ok(mut style) = crosshair.get_single_mut() else { return }; style.display = if state.mode == GameMode::Playing { Display::Flex } else { Display::None }; if let Some(position) = window.cursor_position() { style.left = Val::Px(position.x - 10.0); style.top = Val::Px(position.y - 10.0); } }

fn update_enemy_bars(enemies: Query<(&Enemy, &Children)>, mut bars: Query<(&mut Sprite, &EnemyHealthBar)>) { for (enemy, children) in &enemies { for child in children.iter() { if let Ok((mut sprite, bar)) = bars.get_mut(*child) { let ratio = (enemy.health.max(0) as f32 / enemy.max_health().max(1) as f32).clamp(0.0, 1.0); sprite.custom_size = Some(Vec2::new((enemy.radius * 2.0) * ratio, 5.0)); sprite.color = if ratio < 0.35 { Color::srgb(1.0, 0.15, 0.2) } else { Color::srgb(0.2, 0.95, 0.45) }; let _ = bar.max_health; } } } }

fn floating_text_system(time: Res<Time>, state: Res<GameState>, mut texts: Query<(&mut Transform, &FloatingText)>) { if state.mode != GameMode::Playing { return; } for (mut transform, floating) in &mut texts { transform.translation += floating.velocity.extend(0.0) * time.delta_seconds(); } }

fn update_cooldowns(time: Res<Time>, mut state: ResMut<GameState>) { if state.mode == GameMode::Playing { for cooldown in &mut state.cooldowns { *cooldown = (*cooldown - time.delta_seconds()).max(0.0); } state.speed_boost = (state.speed_boost - time.delta_seconds()).max(0.0); state.damage_cooldown = (state.damage_cooldown - time.delta_seconds()).max(0.0); } state.room_notice = (state.room_notice - time.delta_seconds()).max(0.0); }
fn lifetime_cleanup(mut commands: Commands, time: Res<Time>, state: Res<GameState>, mut entities: Query<(Entity, &mut Lifetime)>) { if state.mode != GameMode::Playing { return; } for (entity, mut lifetime) in &mut entities { lifetime.0 -= time.delta_seconds(); if lifetime.0 <= 0.0 { commands.entity(entity).despawn_recursive(); } } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::GridPos;
    use crate::player::attack_profile;
    use rstest::rstest;

    fn test_enemy(radius: f32) -> Enemy {
        Enemy {
            kind: EnemyKind::Minion,
            health: 18,
            room: IVec2::ZERO,
            radius,
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
    fn game_starts_with_all_nine_rooms_and_empty_inventory() {
        let state = GameState::default();

        assert_eq!(state.rooms.len(), 9);
        assert!(state.rooms.keys().all(|room| room.x.abs() <= 1 && room.y.abs() <= 1));
        assert_eq!(state.room, IVec2::ZERO);
        assert_eq!(state.health, state.max_health);
        assert_eq!(state.mana, state.max_mana);
        assert!(state.inventory.is_empty());
    }

    #[test]
    fn room_walls_have_openings_only_toward_existing_rooms() {
        let center = room_walls(IVec2::ZERO);
        let corner = room_walls(IVec2::new(1, 1));

        // A room with four neighbors has two wall segments on every side.
        assert_eq!(center.iter().filter(|(position, size)| size.y <= 12.0 && position.y.abs() > 280.0).count(), 4);
        assert_eq!(center.iter().filter(|(position, size)| size.x <= 12.0 && position.x.abs() > 530.0).count(), 4);
        // The northeast corner has solid north/east walls and openings only
        // on its south/west sides.
        assert!(corner.iter().any(|(position, size)| position.y > 300.0 && size.x > 1000.0));
        assert!(corner.iter().any(|(position, size)| position.x > 550.0 && size.y > 600.0));
        assert!(corner.iter().filter(|(position, size)| size.y <= 12.0 && position.y.abs() > 280.0).count() < 4);
        assert!(corner.iter().filter(|(position, size)| size.x <= 12.0 && position.x.abs() > 530.0).count() < 4);
    }

    #[test]
    fn navigation_grid_marks_wall_cells_blocked() {
        let mut nav = RoomNavGrid::default();
        nav.rebuild(IVec2::ZERO, &[(Vec2::ZERO, Vec2::new(160.0, 12.0))]);

        assert!(nav.blocked.iter().any(|blocked| *blocked));
        assert!(nav.is_open(GridPos { x: 2, y: 2 }));
        assert!(!nav.is_open(nav.world_to_cell(Vec2::new(0.0, 0.0))));
    }

    #[test]
    fn navigation_allows_diagonals_but_prevents_corner_cutting() {
        let mut nav = RoomNavGrid::default();
        let start = GridPos { x: 5, y: 5 };
        let diagonal = GridPos { x: 6, y: 6 };
        assert!(nav.neighbors(start).contains(&(diagonal, 14)));

        nav.blocked[(5 * nav.width + 6) as usize] = true;
        nav.blocked[(6 * nav.width + 5) as usize] = true;
        assert!(!nav.neighbors(start).contains(&(diagonal, 14)));
    }

    #[rstest]
    #[case(AttackKind::Slash, Vec2::new(80.0, 0.0), 24.0, true)]
    #[case(AttackKind::Slash, Vec2::new(90.0, 0.0), 24.0, false)]
    #[case(AttackKind::Fireball, Vec2::new(40.0, 0.0), 24.0, true)]
    #[case(AttackKind::Fireball, Vec2::new(45.0, 0.0), 18.0, false)]
    #[case(AttackKind::Laser, Vec2::new(70.0, 0.0), 10.0, true)]
    #[case(AttackKind::Laser, Vec2::new(0.0, 70.0), 10.0, false)]
    fn attack_hitboxes_detect_edges_and_respect_direction(
        #[case] kind: AttackKind,
        #[case] enemy_offset: Vec2,
        #[case] enemy_radius: f32,
        #[case] expected: bool,
    ) {
        let attack = Transform::from_translation(Vec3::ZERO);
        assert_eq!(attack_hits_enemy(&attack, kind, enemy_offset, enemy_radius), expected);
    }

    #[test]
    fn inventory_use_consumes_one_item_and_applies_caps() {
        let mut state = GameState::default();
        state.health = 90;
        state.inventory = vec![ItemKind::HealthPotion];
        assert!(consume_item(&mut state, ItemKind::HealthPotion));
        assert_eq!(state.health, 100);
        assert!(state.inventory.is_empty());
    }

    #[test]
    fn player_and_enemy_collision_separates_both_and_deals_damage() {
        let mut app = App::new();
        let mut state = GameState::default();
        state.mode = GameMode::Playing;
        app.insert_resource(state)
            .add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((
            Player { hit_flash: 0.0, health: 100, max_health: 100 },
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world_mut().spawn((
            test_enemy(23.0),
            BounceVelocity(Vec2::ZERO),
            Transform::from_translation(Vec3::new(25.0, 0.0, 0.0)),
        ));

        app.update();

        let (player_x, player_bounce_x, player_hit_flash) = {
            let world = app.world_mut();
            let mut player_query = world.query::<(&Transform, &BounceVelocity, &Player)>();
            let (transform, bounce, player) = player_query.single(world);
            (transform.translation.x, bounce.0.x, player.hit_flash)
        };
        let (enemy_x, enemy_bounce_x) = {
            let world = app.world_mut();
            let mut enemy_query = world.query::<(&Transform, &BounceVelocity, &Enemy)>();
            let (transform, bounce, _) = enemy_query.single(world);
            (transform.translation.x, bounce.0.x)
        };
        let state = app.world().resource::<GameState>();

        assert!(player_x < 0.0);
        assert!(enemy_x > 25.0);
        assert!(player_bounce_x < 0.0);
        assert!(enemy_bounce_x > 0.0);
        assert_eq!(state.health, 90);
        assert!(player_hit_flash > 0.0);
    }

    #[test]
    fn room_layouts_are_deterministic_but_not_all_identical() {
        assert_eq!(room_walls(IVec2::new(-1, 1)), room_walls(IVec2::new(-1, 1)));
        let center = room_walls(IVec2::ZERO);
        assert!((-1..=1).flat_map(|y| (-1..=1).map(move |x| IVec2::new(x, y)))
            .any(|room| room != IVec2::ZERO && room_walls(room) != center));
    }

    #[test]
    fn navigation_converts_world_positions_and_finds_nearby_open_cells() {
        let mut nav = RoomNavGrid::default();
        let cell = GridPos { x: 10, y: 7 };
        assert_eq!(nav.world_to_cell(nav.cell_to_world(cell)), cell);
        assert!(!nav.in_bounds(GridPos { x: -1, y: 0 }));
        assert!(!nav.in_bounds(GridPos { x: nav.width, y: nav.height }));

        nav.blocked[(7 * nav.width + 10) as usize] = true;
        assert_ne!(nav.nearest_open(cell), Some(cell));
        assert!(nav.nearest_open(cell).is_some());
    }

    #[test]
    fn enemy_astar_routes_around_another_enemy_and_uses_diagonals() {
        let nav = RoomNavGrid::default();
        let start = GridPos { x: 2, y: 2 };
        let goal = GridPos { x: 10, y: 8 };
        let blocker = Entity::from_raw(99);
        let occupied = [(blocker, nav.cell_to_world(GridPos { x: 6, y: 5 }), 23.0)];
        let path = navigation::find_enemy_path(&nav, start, goal, Entity::from_raw(1), &occupied).expect("path should exist");

        assert_eq!(path.first(), Some(&start));
        assert_eq!(path.last(), Some(&goal));
        assert!(path.windows(2).any(|step| (step[1].x - step[0].x).abs() == 1 && (step[1].y - step[0].y).abs() == 1));
        assert!(!path.iter().any(|cell| *cell == GridPos { x: 6, y: 5 }));
    }

    #[rstest]
    #[case(AttackKind::Laser, 5, 900.0, 8, 0.25)]
    #[case(AttackKind::Slash, 8, 0.0, 0, 0.45)]
    #[case(AttackKind::Fireball, 7, 330.0, 8, 0.8)]
    fn attack_profiles_match_design(#[case] kind: AttackKind, #[case] damage: i32, #[case] speed: f32, #[case] mana: i32, #[case] cooldown: f32) {
        let profile = attack_profile(kind);
        assert_eq!(profile.damage, damage);
        assert_eq!(profile.speed, speed);
        assert_eq!(profile.mana_cost, mana);
        assert_eq!(profile.cooldown, cooldown);
    }

    #[test]
    fn projectile_wall_overlap_stops_at_the_first_wall() {
        let wall_center = Vec2::new(100.0, 0.0);
        let wall_size = Vec2::new(20.0, 120.0);
        assert!(!circle_hits_wall(Vec2::new(70.0, 0.0), 6.0, wall_center, wall_size));
        assert!(circle_hits_wall(Vec2::new(89.0, 0.0), 6.0, wall_center, wall_size));
        assert!(!circle_hits_wall(Vec2::new(89.0, 70.0), 6.0, wall_center, wall_size));
    }

    #[test]
    fn wall_collision_pushes_player_out_instead_of_through() {
        let mut app = App::new();
        let mut state = GameState::default();
        state.mode = GameMode::Playing;
        app.insert_resource(state);
        app.add_systems(Update, wall::wall_collision);
        app.world_mut().spawn((Player { hit_flash: 0.0, health: 100, max_health: 100 }, Transform::from_xyz(0.0, 0.0, 0.0)));
        app.world_mut().spawn((Wall { size: Vec2::new(20.0, 100.0) }, Transform::from_xyz(20.0, 0.0, 0.0)));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Transform, With<Player>>();
        assert!(query.single(app.world()).translation.x < 0.0);
    }

    #[test]
    fn pickup_collection_adds_items_and_respects_inventory_capacity() {
        let mut state = GameState::default();
        state.mode = GameMode::Playing;
        state.inventory = vec![ItemKind::ManaPotion; 23];
        let mut app = App::new();
        app.insert_resource(state).add_systems(Update, collect_pickups);
        app.world_mut().spawn((Player { hit_flash: 0.0, health: 100, max_health: 100 }, Transform::from_translation(Vec3::ZERO)));
        let near = app.world_mut().spawn((Pickup { item: ItemKind::HealthPotion }, Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)))).id();
        let far = app.world_mut().spawn((Pickup { item: ItemKind::SpeedPotion }, Transform::from_translation(Vec3::new(100.0, 0.0, 0.0)))).id();
        app.update();

        assert_eq!(app.world().resource::<GameState>().inventory.len(), 24);
        assert!(!app.world().get_entity(near).is_some_and(|entity| entity.contains::<Pickup>()));
        assert!(app.world().get_entity(far).is_some_and(|entity| entity.contains::<Pickup>()));
    }

    #[test]
    fn each_potion_has_its_expected_effect_and_missing_items_are_ignored() {
        let mut state = GameState::default();
        state.health = 80;
        state.mana = 70;
        state.inventory = vec![ItemKind::HealthPotion, ItemKind::ManaPotion, ItemKind::SpeedPotion];
        assert!(consume_item(&mut state, ItemKind::HealthPotion));
        assert!(consume_item(&mut state, ItemKind::ManaPotion));
        assert!(consume_item(&mut state, ItemKind::SpeedPotion));
        assert!(!consume_item(&mut state, ItemKind::HealthPotion));
        assert_eq!(state.health, 100);
        assert_eq!(state.mana, 100);
        assert_eq!(state.speed_boost, 25.0);
        assert!(state.inventory.is_empty());
    }

    #[test]
    fn repeated_enemy_contact_respects_damage_cooldown() {
        let mut app = App::new();
        let mut state = GameState::default();
        state.mode = GameMode::Playing;
        state.damage_cooldown = 0.5;
        app.insert_resource(state).add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((Player { hit_flash: 0.0, health: 100, max_health: 100 }, BounceVelocity(Vec2::ZERO), Transform::from_translation(Vec3::ZERO)));
        app.world_mut().spawn((test_enemy(23.0), BounceVelocity(Vec2::ZERO), Transform::from_translation(Vec3::new(25.0, 0.0, 0.0))));
        app.update();
        assert_eq!(app.world().resource::<GameState>().health, 100);
    }

    #[test]
    fn projectile_combat_damages_once_flashes_enemy_despawns_projectile_and_clears_room() {
        let mut app = App::new();
        let mut state = GameState::default();
        state.mode = GameMode::Playing;
        app.insert_resource(state)
            .insert_resource(Time::<()>::default())
            .add_systems(Update, animate_attacks);
        let room = IVec2::new(1, -1);
        let enemy = app.world_mut().spawn((
            test_enemy(23.0),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        )).id();
        let projectile = app.world_mut().spawn((
            Attack { kind: AttackKind::Laser, damage: 5, velocity: Vec2::ZERO, age: 0.0, hit_targets: Vec::new() },
            Transform::from_translation(Vec3::ZERO),
        )).id();
        app.world_mut().entity_mut(enemy).get_mut::<Enemy>().unwrap().room = room;

        app.update();

        let enemy_data = app.world().get::<Enemy>(enemy).expect("enemy survives one hit");
        assert_eq!(enemy_data.health, 13);
        assert_eq!(enemy_data.hit_flash, 0.225);
        assert!(!app.world().get_entity(projectile).is_some_and(|entity| entity.contains::<Attack>()));

        app.world_mut().entity_mut(enemy).get_mut::<Enemy>().unwrap().health = 5;
        let finishing_projectile = app.world_mut().spawn((
            Attack { kind: AttackKind::Laser, damage: 5, velocity: Vec2::ZERO, age: 0.0, hit_targets: Vec::new() },
            Transform::from_translation(Vec3::ZERO),
        )).id();
        app.update();

        assert!(!app.world().get_entity(finishing_projectile).is_some_and(|entity| entity.contains::<Attack>()));
        assert!(app.world().get::<Enemy>(enemy).is_none());
        assert_eq!(app.world().resource::<GameState>().rooms.get(&room), Some(&true));
    }
}
