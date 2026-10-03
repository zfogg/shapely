use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy_rapier2d::prelude::*;
use pathfinding::prelude::astar;
use std::collections::HashMap;

const ROOM_SIZE: Vec2 = Vec2::new(1120.0, 620.0);
const PLAYER_SPEED: f32 = 280.0;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.09)))
        .insert_resource(GameState::default())
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
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_systems(Startup, setup)
        .add_systems(Update, (
            player_movement,
            wall_collision,
            attack_input,
            animate_attacks,
            enemy_flash,
            enemy_ai,
            enemy_player_collision,
            player_flash,
            collect_pickups,
            update_room,
            toggle_inventory,
            use_inventory_items,
            update_hud,
            update_cooldowns,
            lifetime_cleanup,
        ).chain())
        .run();
}

#[derive(Resource)]
struct GameState {
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
}

impl Default for GameState {
    fn default() -> Self {
        let mut rooms = HashMap::new();
        for y in -1..=1 { for x in -1..=1 { rooms.insert(IVec2::new(x, y), false); } }
        Self { room: IVec2::ZERO, health: 100, mana: 100, max_health: 100, max_mana: 100,
            speed_boost: 0.0, damage_cooldown: 0.0, inventory_open: false, inventory: Vec::new(), cooldowns: [0.0; 3], rooms }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind { HealthPotion, ManaPotion, SpeedPotion }

#[derive(Component)] struct Player { hit_flash: f32 }
#[derive(Component)] struct BounceVelocity(Vec2);
#[derive(Component)] struct Enemy { kind: EnemyKind, health: i32, room: IVec2, radius: f32, phase: f32, path: Vec<GridPos>, path_index: usize, planned_goal: Option<GridPos>, repath_timer: f32, hit_flash: f32, base_color: Color }
#[derive(Clone, Copy)] enum EnemyKind { Minion, Thug, Miniboss }
#[derive(Component)] struct Attack { kind: AttackKind, damage: i32, velocity: Vec2, age: f32, hit_targets: Vec<Entity> }
#[derive(Clone, Copy, PartialEq)] enum AttackKind { Laser, Slash, Fireball }
#[derive(Component)] struct Pickup { item: ItemKind }
#[derive(Component)] struct Lifetime(f32);
#[derive(Component)] struct Hud;
#[derive(Component)] struct HealthBarFill;
#[derive(Component)] struct InventoryPanel;
#[derive(Component)] struct InventoryText;
#[derive(Component)] struct RoomEntity;
#[derive(Component)] struct Wall { size: Vec2 }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)] struct GridPos { x: i32, y: i32 }

#[derive(Resource)]
struct RoomNavGrid { room: IVec2, width: i32, height: i32, cell_size: f32, origin: Vec2, blocked: Vec<bool> }

impl Default for RoomNavGrid {
    fn default() -> Self { Self { room: IVec2::ZERO, width: 28, height: 16, cell_size: 40.0, origin: Vec2::new(-560.0, -320.0), blocked: vec![false; 28 * 16] } }
}

impl RoomNavGrid {
    fn index(&self, cell: GridPos) -> Option<usize> { if self.in_bounds(cell) { Some((cell.y * self.width + cell.x) as usize) } else { None } }
    fn in_bounds(&self, cell: GridPos) -> bool { cell.x >= 0 && cell.y >= 0 && cell.x < self.width && cell.y < self.height }
    fn is_open(&self, cell: GridPos) -> bool { self.index(cell).map(|index| !self.blocked[index]).unwrap_or(false) }
    fn world_to_cell(&self, position: Vec2) -> GridPos { GridPos { x: ((position.x - self.origin.x) / self.cell_size).floor() as i32, y: ((position.y - self.origin.y) / self.cell_size).floor() as i32 } }
    fn cell_to_world(&self, cell: GridPos) -> Vec2 { self.origin + Vec2::new((cell.x as f32 + 0.5) * self.cell_size, (cell.y as f32 + 0.5) * self.cell_size) }
    fn nearest_open(&self, start: GridPos) -> Option<GridPos> { for radius in 0..self.width.max(self.height) { for y in (start.y - radius)..=(start.y + radius) { for x in (start.x - radius)..=(start.x + radius) { let cell = GridPos { x, y }; if self.is_open(cell) { return Some(cell); } } } } None }
    fn neighbors(&self, cell: GridPos) -> Vec<(GridPos, usize)> {
        let mut result = Vec::new();
        for (dx, dy, cost) in [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (-1, 1, 14), (1, -1, 14), (-1, -1, 14)] {
            let candidate = GridPos { x: cell.x + dx, y: cell.y + dy };
            if !self.is_open(candidate) { continue; }
            // Do not cut diagonally through the corner of two blocked cells.
            if dx != 0 && dy != 0 && (!self.is_open(GridPos { x: cell.x + dx, y: cell.y }) || !self.is_open(GridPos { x: cell.x, y: cell.y + dy })) { continue; }
            result.push((candidate, cost));
        }
        result
    }
    fn rebuild(&mut self, room: IVec2, walls: &[(Vec2, Vec2)]) {
        self.room = room;
        self.blocked.fill(false);
        let clearance = 36.0;
        for y in 0..self.height { for x in 0..self.width { let center = self.cell_to_world(GridPos { x, y }); self.blocked[(y * self.width + x) as usize] = walls.iter().any(|(wall_center, wall_size)| { let delta = center - *wall_center; let overlap = Vec2::splat(clearance) + *wall_size * 0.5 - delta.abs(); overlap.x > 0.0 && overlap.y > 0.0 }); } }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, nav: ResMut<RoomNavGrid>) {
    commands.spawn(Camera2dBundle::default());
    // Shapely is a top-down game: the screen plane is the floor. Keep Rapier
    // collision support, but never let gravity pull the player out of the room.
    commands.spawn((Player { hit_flash: 0.0 }, BounceVelocity(Vec2::ZERO), RigidBody::KinematicPositionBased, Collider::cuboid(18.0, 18.0),
        SpriteBundle { sprite: Sprite { color: Color::srgb(1.0, 0.12, 0.4), custom_size: Some(Vec2::splat(38.0)), ..default() }, transform: Transform::from_xyz(0.0, 0.0, 5.0), ..default() }));
    build_room(&mut commands, &mut meshes, &mut materials, IVec2::ZERO, true, nav);
    spawn_hud(&mut commands, &asset_server);
}

fn spawn_hud(commands: &mut Commands, assets: &Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((Hud, TextBundle { text: Text::from_section("", TextStyle { font: font.clone(), font_size: 18.0, color: Color::WHITE }), style: Style { position_type: PositionType::Absolute, left: Val::Px(24.0), top: Val::Px(18.0), ..default() }, ..default() }));
    commands.spawn(NodeBundle { style: Style { position_type: PositionType::Absolute, left: Val::Px(24.0), top: Val::Px(72.0), width: Val::Px(220.0), height: Val::Px(18.0), ..default() }, background_color: BackgroundColor(Color::srgba(0.12, 0.02, 0.05, 0.95)), ..default() }).with_children(|parent| {
        parent.spawn((HealthBarFill, NodeBundle { style: Style { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, background_color: BackgroundColor(Color::srgb(1.0, 0.12, 0.25)), ..default() }));
    });
    commands.spawn((InventoryPanel, NodeBundle { style: Style { display: Display::None, position_type: PositionType::Absolute, left: Val::Percent(18.0), top: Val::Percent(12.0), width: Val::Percent(64.0), height: Val::Percent(70.0), padding: UiRect::all(Val::Px(28.0)), flex_direction: FlexDirection::Column, ..default() }, background_color: BackgroundColor(Color::srgba(0.04, 0.055, 0.14, 0.97)), ..default() })).with_children(|parent| {
        parent.spawn((InventoryText, TextBundle { text: Text::from_section("INVENTORY // 24 SLOTS", TextStyle { font, font_size: 24.0, color: Color::srgb(0.3, 0.9, 1.0) }), ..default() }));
    });
}

fn room_seed(room: IVec2) -> u32 {
    let x = room.x as u32;
    let y = room.y as u32;
    x.wrapping_mul(0x9E37_79B9).rotate_left(13) ^ y.wrapping_mul(0x85EB_CA6B).rotate_right(7) ^ 0xC0FF_EE11
}

fn spawn_wall(commands: &mut Commands, position: Vec2, size: Vec2, color: Color) {
    commands.spawn((RoomEntity, Wall { size }, Collider::cuboid(size.x * 0.5, size.y * 0.5), SpriteBundle {
        sprite: Sprite { color, custom_size: Some(size), ..default() },
        transform: Transform::from_xyz(position.x, position.y, 1.0), ..default()
    }));
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

fn build_room(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<ColorMaterial>>, room: IVec2, populated: bool, mut nav: ResMut<RoomNavGrid>) {
    let seed = room_seed(room);
    commands.spawn((RoomEntity, SpriteBundle { sprite: Sprite { color: Color::srgb(0.035 + (seed % 4) as f32 * 0.006, 0.045, 0.12 + (seed % 3) as f32 * 0.008), custom_size: Some(ROOM_SIZE), ..default() }, transform: Transform::from_xyz(0.0, 0.0, -2.0), ..default() }));
    let wall_color = Color::srgb(0.18, 0.22, 0.5);
    let walls = room_walls(room);
    for (position, size) in &walls { spawn_wall(commands, *position, *size, if size.x > size.y { wall_color } else { Color::srgb(0.12, 0.28, 0.48) }); }
    nav.rebuild(room, &walls);
    let accents = [(-380.0, 190.0), (360.0, 155.0), (-280.0, -190.0), (260.0, -190.0)];
    for (index, (x, y)) in accents.iter().enumerate() { if (seed + index as u32) % 3 != 0 { commands.spawn((RoomEntity, MaterialMesh2dBundle { mesh: meshes.add(Rectangle::new(90.0, 5.0)).into(), material: materials.add(ColorMaterial::from(Color::srgb(0.06, 0.8, 0.92))), transform: Transform::from_xyz(*x, *y, -1.0), ..default() })); } }
    if populated { spawn_enemies(commands, meshes, materials, room); }
}

fn spawn_enemies(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<ColorMaterial>>, room: IVec2) {
    let shift = (room_seed(room) % 5) as f32 * 18.0;
    let specs = [(EnemyKind::Minion, Vec2::new(-260.0 + shift, 120.0)), (EnemyKind::Minion, Vec2::new(250.0 - shift, -80.0)), (EnemyKind::Thug, Vec2::new(180.0, 180.0 - shift)), (EnemyKind::Miniboss, Vec2::new(0.0, -160.0 + shift))];
    for (kind, pos) in specs {
        let (mesh, color, hp) = match kind { EnemyKind::Minion => (meshes.add(Circle::new(23.0)), Color::srgb(0.2, 0.9, 0.72), 18), EnemyKind::Thug => (meshes.add(RegularPolygon::new(30.0, 6)), Color::srgb(1.0, 0.48, 0.16), 34), EnemyKind::Miniboss => (meshes.add(RegularPolygon::new(48.0, 8)), Color::srgb(0.78, 0.25, 1.0), 80) };
        let radius = match kind { EnemyKind::Minion => 23.0, EnemyKind::Thug => 30.0, EnemyKind::Miniboss => 48.0 };
        commands.spawn((RoomEntity, BounceVelocity(Vec2::ZERO), Enemy { kind, health: hp, room, radius, phase: pos.x * 0.01, path: Vec::new(), path_index: 0, planned_goal: None, repath_timer: 0.0, hit_flash: 0.0, base_color: color }, MaterialMesh2dBundle { mesh: mesh.into(), material: materials.add(ColorMaterial::from(color)), transform: Transform::from_xyz(pos.x, pos.y, 3.0), ..default() }));
    }
}

fn player_movement(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, state: Res<GameState>, mut query: Query<(&mut Transform, &mut BounceVelocity), With<Player>>) {
    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { direction.y += 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { direction.y -= 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { direction.x -= 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { direction.x += 1.0; }
    if let Ok((mut transform, mut bounce)) = query.get_single_mut() {
        if bounce.0.length_squared() > 1.0 { transform.translation += bounce.0.extend(0.0) * time.delta_seconds(); bounce.0 = bounce.0.lerp(Vec2::ZERO, (10.0 * time.delta_seconds()).min(1.0)); return; }
        let speed = PLAYER_SPEED * if state.speed_boost > 0.0 { 1.55 } else { 1.0 }; transform.translation += direction.normalize_or_zero().extend(0.0) * speed * time.delta_seconds(); transform.translation.x = transform.translation.x.clamp(-570.0, 570.0); transform.translation.y = transform.translation.y.clamp(-320.0, 320.0);
    }
}

fn wall_collision(mut player: Query<&mut Transform, With<Player>>, walls: Query<(&Transform, &Wall), Without<Player>>) {
    let Ok(mut player) = player.get_single_mut() else { return };
    let player_half = Vec2::splat(19.0);
    for (wall_transform, wall) in &walls {
        let delta = player.translation.truncate() - wall_transform.translation.truncate();
        let overlap = player_half + wall.size * 0.5 - delta.abs();
        if overlap.x > 0.0 && overlap.y > 0.0 {
            if overlap.x < overlap.y {
                player.translation.x += if delta.x >= 0.0 { overlap.x } else { -overlap.x };
            } else {
                player.translation.y += if delta.y >= 0.0 { overlap.y } else { -overlap.y };
            }
        }
    }
}

fn attack_input(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut state: ResMut<GameState>, player: Query<&Transform, With<Player>>, windows: Query<&Window>, cameras: Query<(&Camera, &GlobalTransform)>) {
    let Ok(player) = player.get_single() else { return };
    let mut requested = None;
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Space) { requested = Some(AttackKind::Laser); }
    if keys.just_pressed(KeyCode::Digit2) { requested = Some(AttackKind::Slash); }
    if keys.just_pressed(KeyCode::Digit3) { requested = Some(AttackKind::Fireball); }
    let Some(kind) = requested else { return };
    let index = match kind { AttackKind::Laser => 0, AttackKind::Slash => 1, AttackKind::Fireball => 2 };
    if state.cooldowns[index] > 0.0 { return; }
    let profile = attack_profile(kind);
    let mana_cost = profile.mana_cost;
    if state.mana < mana_cost { return; } state.mana -= mana_cost;
    let (camera, camera_transform) = cameras.single();
    let Some(cursor) = windows.single().cursor_position().and_then(|position| camera.viewport_to_world_2d(camera_transform, position)) else { return };
    let direction = (cursor - player.translation.truncate()).normalize_or_zero();
    if direction == Vec2::ZERO { return; }
    let velocity = direction * profile.speed;
    state.cooldowns[index] = profile.cooldown;
    let spawn_position = player.translation + direction.extend(0.0) * 42.0 + Vec3::new(0.0, 0.0, 4.0);
    commands.spawn((Attack { kind, damage: profile.damage, velocity, age: 0.0, hit_targets: Vec::new() }, Lifetime(profile.max_age), SpriteBundle { sprite: Sprite { color: profile.color, custom_size: Some(profile.size), ..default() }, transform: Transform { translation: spawn_position, rotation: Quat::from_rotation_z(direction.y.atan2(direction.x)), ..default() }, ..default() }));
    let _ = time;
}

struct AttackProfile { damage: i32, speed: f32, max_age: f32, size: Vec2, color: Color, mana_cost: i32, cooldown: f32 }

fn attack_profile(kind: AttackKind) -> AttackProfile {
    match kind {
        AttackKind::Laser => AttackProfile { damage: 5, speed: 900.0, max_age: 1.2, size: Vec2::new(130.0, 8.0), color: Color::srgb(0.2, 0.95, 1.0), mana_cost: 8, cooldown: 0.25 },
        AttackKind::Slash => AttackProfile { damage: 8, speed: 0.0, max_age: 0.22, size: Vec2::splat(120.0), color: Color::srgb(1.0, 0.25, 0.6), mana_cost: 0, cooldown: 0.45 },
        AttackKind::Fireball => AttackProfile { damage: 7, speed: 330.0, max_age: 1.5, size: Vec2::splat(34.0), color: Color::srgb(1.0, 0.38, 0.08), mana_cost: 8, cooldown: 0.8 },
    }
}

fn animate_attacks(time: Res<Time>, mut attacks: Query<(Entity, &mut Transform, &mut Attack), Without<Enemy>>, mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Attack>>, walls: Query<(&Transform, &Wall), (Without<Attack>, Without<Enemy>)>, mut commands: Commands, mut state: ResMut<GameState>) {
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
        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies { let can_hit = (attack.kind != AttackKind::Slash || attack.age < 0.12 + time.delta_seconds()) && !attack.hit_targets.contains(&enemy_entity) && attack_hits_enemy(&transform, attack.kind, enemy_transform.translation.truncate(), enemy.radius); if can_hit { attack.hit_targets.push(enemy_entity); enemy.health -= attack.damage; enemy.hit_flash = 0.225; if enemy.health <= 0 { let drop = match enemy.kind { EnemyKind::Minion => ItemKind::ManaPotion, EnemyKind::Thug => ItemKind::HealthPotion, EnemyKind::Miniboss => ItemKind::SpeedPotion }; killed_rooms.push(enemy.room); commands.spawn((RoomEntity, Pickup { item: drop }, SpriteBundle { sprite: Sprite { color: item_color(drop), custom_size: Some(Vec2::splat(22.0)), ..default() }, transform: *enemy_transform, ..default() })); commands.entity(enemy_entity).despawn_recursive(); } if attack.kind != AttackKind::Slash { hit_enemy = true; } break; } }
        if hit_enemy { commands.entity(attack_entity).despawn_recursive(); }
    }
    killed_rooms.sort_by_key(|room| (room.x, room.y));
    killed_rooms.dedup();
    for room in killed_rooms { if !enemies.iter().any(|(_, enemy, _)| enemy.room == room && enemy.health > 0) { state.rooms.insert(room, true); } }
}

fn attack_hits_enemy(attack_transform: &Transform, kind: AttackKind, enemy_position: Vec2, enemy_radius: f32) -> bool {
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

fn circle_hits_wall(position: Vec2, radius: f32, wall_center: Vec2, wall_size: Vec2) -> bool {
    let delta = position - wall_center;
    let overlap = Vec2::splat(radius) + wall_size * 0.5 - delta.abs();
    overlap.x > 0.0 && overlap.y > 0.0
}

fn enemy_flash(time: Res<Time>, mut enemies: Query<(&mut Enemy, &Handle<ColorMaterial>)>, mut materials: ResMut<Assets<ColorMaterial>>) {
    for (mut enemy, material_handle) in &mut enemies {
        enemy.hit_flash = (enemy.hit_flash - time.delta_seconds()).max(0.0);
        if let Some(material) = materials.get_mut(material_handle) { material.color = if enemy.hit_flash > 0.0 { Color::srgb(1.0, 0.03, 0.03) } else { enemy.base_color }; }
    }
}

fn item_color(item: ItemKind) -> Color { match item { ItemKind::HealthPotion => Color::srgb(1.0, 0.15, 0.25), ItemKind::ManaPotion => Color::srgb(0.2, 0.55, 1.0), ItemKind::SpeedPotion => Color::srgb(0.9, 0.2, 1.0) } }

fn enemy_ai(time: Res<Time>, mut enemy_queries: ParamSet<(Query<(Entity, &mut Transform, &mut Enemy, &mut BounceVelocity), Without<Player>>, Query<(Entity, &Transform, &Enemy), With<Enemy>>)>, player: Query<&Transform, (With<Player>, Without<Enemy>)>, nav: Res<RoomNavGrid>) {
    let Ok(player) = player.get_single() else { return };
    let goal = nav.world_to_cell(player.translation.truncate());
    let Some(goal) = nav.nearest_open(goal) else { return };
    let occupied_enemies: Vec<(Entity, Vec2, f32)> = enemy_queries.p1().iter().map(|(entity, transform, enemy)| (entity, transform.translation.truncate(), enemy.radius)).collect();
    for (entity, mut transform, mut enemy, mut bounce) in &mut enemy_queries.p0() {
        enemy.phase += time.delta_seconds();
        if bounce.0.length_squared() > 1.0 { transform.translation += bounce.0.extend(0.0) * time.delta_seconds(); bounce.0 = bounce.0.lerp(Vec2::ZERO, (10.0 * time.delta_seconds()).min(1.0)); continue; }
        let to_player = (player.translation - transform.translation).truncate();
        if to_player.length() >= 58.0 {
            let start = nav.nearest_open(nav.world_to_cell(transform.translation.truncate()));
            enemy.repath_timer = (enemy.repath_timer - time.delta_seconds()).max(0.0);
            if enemy.planned_goal != Some(goal) || enemy.path_index >= enemy.path.len() || enemy.repath_timer <= 0.0 { if let Some(start) = start { if let Some(path) = find_enemy_path(&nav, start, goal, entity, &occupied_enemies) { enemy.path = path; enemy.path_index = 1; enemy.planned_goal = Some(goal); enemy.repath_timer = 0.2; } } }
            if enemy.path_index < enemy.path.len() {
                let waypoint = nav.cell_to_world(enemy.path[enemy.path_index]);
                let direction = (waypoint - transform.translation.truncate()).normalize_or_zero();
                let pace = match enemy.kind { EnemyKind::Minion => 22.0, EnemyKind::Thug => 14.0, EnemyKind::Miniboss => 9.0 };
                transform.translation += direction.extend(0.0) * pace * time.delta_seconds();
                if transform.translation.truncate().distance(waypoint) < 8.0 { enemy.path_index += 1; }
            }
        }
        transform.rotation = Quat::from_rotation_z((enemy.phase * 2.0).sin() * 0.08);
    }
}

fn find_enemy_path(nav: &RoomNavGrid, start: GridPos, goal: GridPos, entity: Entity, occupied_enemies: &[(Entity, Vec2, f32)]) -> Option<Vec<GridPos>> {
    let successors = |cell: &GridPos| nav.neighbors(*cell).into_iter().filter(|(next, _)| {
        if *next == goal || *next == start { return true; }
        let center = nav.cell_to_world(*next);
        !occupied_enemies.iter().any(|(other_entity, other_position, other_radius)| *other_entity != entity && center.distance(*other_position) < *other_radius + 28.0)
    }).collect::<Vec<_>>();
    astar(&start, successors, |cell| { let dx = (cell.x - goal.x).unsigned_abs() as usize; let dy = (cell.y - goal.y).unsigned_abs() as usize; dx.max(dy) * 10 + dx.min(dy) * 4 }, |cell| *cell == goal).map(|(path, _)| path)
}

fn enemy_player_collision(mut player: Query<(&mut Transform, &mut BounceVelocity, &mut Player)>, mut enemies: Query<(&mut Transform, &mut BounceVelocity, &Enemy), Without<Player>>, mut state: ResMut<GameState>) {
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
            player_bounce.0 = normal * 260.0;
            enemy_bounce.0 = -normal * 220.0;
            player_data.hit_flash = 0.225;
            if state.damage_cooldown <= 0.0 {
                state.health = (state.health - 10).max(0);
                state.damage_cooldown = 0.55;
            }
        }
    }
}

fn player_flash(time: Res<Time>, mut player: Query<(&mut Player, &mut Sprite)>) {
    let Ok((mut player, mut sprite)) = player.get_single_mut() else { return };
    player.hit_flash = (player.hit_flash - time.delta_seconds()).max(0.0);
    sprite.color = if player.hit_flash > 0.0 { Color::BLACK } else { Color::srgb(1.0, 0.12, 0.4) };
}

fn collect_pickups(mut commands: Commands, mut state: ResMut<GameState>, player: Query<&Transform, (With<Player>, Without<Pickup>)>, pickups: Query<(Entity, &Pickup, &Transform), Without<Player>>) { let Ok(player) = player.get_single() else { return }; for (entity, pickup, transform) in &pickups { if player.translation.distance(transform.translation) < 38.0 { if state.inventory.len() < 24 { state.inventory.push(pickup.item); commands.entity(entity).despawn_recursive(); } } } }

fn update_room(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>, mut player: Query<&mut Transform, With<Player>>, old_room: Query<Entity, With<RoomEntity>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ColorMaterial>>, nav: ResMut<RoomNavGrid>) { let Ok(mut player) = player.get_single_mut() else { return }; let mut next = state.room; if player.translation.x < -548.0 { next.x -= 1; player.translation.x = 520.0; } if player.translation.x > 548.0 { next.x += 1; player.translation.x = -520.0; } if player.translation.y < -298.0 { next.y -= 1; player.translation.y = 270.0; } if player.translation.y > 298.0 { next.y += 1; player.translation.y = -270.0; } if next.x.abs() > 1 || next.y.abs() > 1 { player.translation.x = player.translation.x.clamp(-525.0, 525.0); player.translation.y = player.translation.y.clamp(-275.0, 275.0); return; } if next != state.room { for entity in &old_room { commands.entity(entity).despawn_recursive(); } state.room = next; let populated = !*state.rooms.get(&next).unwrap_or(&false); build_room(&mut commands, &mut meshes, &mut materials, next, populated, nav); } let _ = keys; }

fn toggle_inventory(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>, mut panel: Query<&mut Style, With<InventoryPanel>>) { if keys.just_pressed(KeyCode::KeyI) || keys.just_pressed(KeyCode::Escape) { state.inventory_open = if keys.just_pressed(KeyCode::Escape) { false } else { !state.inventory_open }; if let Ok(mut style) = panel.get_single_mut() { style.display = if state.inventory_open { Display::Flex } else { Display::None }; } } }

fn use_inventory_items(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>) {
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

fn update_hud(state: Res<GameState>, mut hud: Query<&mut Text, (With<Hud>, Without<InventoryText>)>, mut inventory_text: Query<&mut Text, (With<InventoryText>, Without<Hud>)>, mut health_bar: Query<&mut Style, With<HealthBarFill>>) { let inv = state.inventory.iter().take(6).map(|i| match i { ItemKind::HealthPotion => "♥", ItemKind::ManaPotion => "◆", ItemKind::SpeedPotion => "✦" }).collect::<Vec<_>>().join(" "); if let Ok(mut text) = hud.get_single_mut() { text.sections[0].value = format!("SHAPELY  //  ROOM {:+},{:+}\nHP {:>3}/{}   MANA {:>3}/{}   [1] LASER  [2] SLASH  [3] FIREBALL\nQUICK SLOTS: {}   [I] INVENTORY   [H] HEALTH  [M] MANA  [B] BOOST", state.room.x, state.room.y, state.health, state.max_health, state.mana, state.max_mana, if inv.is_empty() { "—".into() } else { inv }); } if let Ok(mut text) = inventory_text.get_single_mut() { let slots = (0..24).map(|slot| match state.inventory.get(slot) { Some(ItemKind::HealthPotion) => "[♥ HP]", Some(ItemKind::ManaPotion) => "[◆ MP]", Some(ItemKind::SpeedPotion) => "[✦ SPD]", None => "[     ]" }).collect::<Vec<_>>().join("  "); text.sections[0].value = format!("INVENTORY // 24 SLOTS\n\n{}\n\nH health   M mana   B speed (25 seconds)\nI / Esc close", slots); } if let Ok(mut style) = health_bar.get_single_mut() { style.width = Val::Percent((state.health.max(0) as f32 / state.max_health as f32) * 100.0); } }

fn update_cooldowns(time: Res<Time>, mut state: ResMut<GameState>) { for cooldown in &mut state.cooldowns { *cooldown = (*cooldown - time.delta_seconds()).max(0.0); } state.speed_boost = (state.speed_boost - time.delta_seconds()).max(0.0); state.damage_cooldown = (state.damage_cooldown - time.delta_seconds()).max(0.0); }
fn lifetime_cleanup(mut commands: Commands, time: Res<Time>, mut entities: Query<(Entity, &mut Lifetime)>) { for (entity, mut lifetime) in &mut entities { lifetime.0 -= time.delta_seconds(); if lifetime.0 <= 0.0 { commands.entity(entity).despawn_recursive(); } } }

#[cfg(test)]
mod tests {
    use super::*;
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
        app.insert_resource(GameState::default())
            .add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((
            Player { hit_flash: 0.0 },
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
        let path = find_enemy_path(&nav, start, goal, Entity::from_raw(1), &occupied).expect("path should exist");

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
        app.add_systems(Update, wall_collision);
        app.world_mut().spawn((Player { hit_flash: 0.0 }, Transform::from_xyz(0.0, 0.0, 0.0)));
        app.world_mut().spawn((Wall { size: Vec2::new(20.0, 100.0) }, Transform::from_xyz(20.0, 0.0, 0.0)));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Transform, With<Player>>();
        assert!(query.single(app.world()).translation.x < 0.0);
    }

    #[test]
    fn pickup_collection_adds_items_and_respects_inventory_capacity() {
        let mut state = GameState::default();
        state.inventory = vec![ItemKind::ManaPotion; 23];
        let mut app = App::new();
        app.insert_resource(state).add_systems(Update, collect_pickups);
        app.world_mut().spawn((Player { hit_flash: 0.0 }, Transform::from_translation(Vec3::ZERO)));
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
        state.damage_cooldown = 0.5;
        app.insert_resource(state).add_systems(Update, enemy_player_collision);
        app.world_mut().spawn((Player { hit_flash: 0.0 }, BounceVelocity(Vec2::ZERO), Transform::from_translation(Vec3::ZERO)));
        app.world_mut().spawn((test_enemy(23.0), BounceVelocity(Vec2::ZERO), Transform::from_translation(Vec3::new(25.0, 0.0, 0.0))));
        app.update();
        assert_eq!(app.world().resource::<GameState>().health, 100);
    }

    #[test]
    fn projectile_combat_damages_once_flashes_enemy_despawns_projectile_and_clears_room() {
        let mut app = App::new();
        app.insert_resource(GameState::default())
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
