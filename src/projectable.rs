//! Projectile components and projectile spawning primitives.
//!
//! Hostile projectiles use the same wall-aware movement loop as other
//! projectiles, making this module the natural home for future projectile
//! types and projectile-specific shaders.

use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::sprite::MaterialMesh2dBundle;
use bevy::sprite::{Material2d, Material2dPlugin};

#[derive(Component)]
/// A projectile emitted by a thug or miniboss.
pub struct HostileProjectile {
    pub velocity: Vec2,
    pub damage: i32,
    pub radius: f32,
    pub kind: FxKind,
}

const THUG_ACCELERATION: f32 = 90.0;
const THUG_MAX_SPEED: f32 = 330.0;
const MINIBOSS_DECELERATION: f32 = 45.0;
const MINIBOSS_MIN_SPEED: f32 = 72.0;

/// Adjusts hostile projectile speed with a finite cap or floor.
pub fn update_hostile_speed(projectile: &mut HostileProjectile, delta_seconds: f32) {
    let speed = projectile.velocity.length();
    let adjusted = match projectile.kind {
        FxKind::ThugShot => (speed + THUG_ACCELERATION * delta_seconds).min(THUG_MAX_SPEED),
        FxKind::MinibossBurst => {
            (speed - MINIBOSS_DECELERATION * delta_seconds).max(MINIBOSS_MIN_SPEED)
        }
        _ => speed,
    };
    projectile.velocity = projectile.velocity.normalize_or_zero() * adjusted;
}

/// Persisted hostile projectile state while the player is visiting another room.
#[derive(Clone)]
pub(crate) struct ProjectileSnapshot {
    pub(crate) kind: FxKind,
    pub(crate) position: Vec3,
    pub(crate) velocity: Vec2,
    pub(crate) damage: i32,
    pub(crate) radius: f32,
    pub(crate) lifetime: f32,
}

/// Visual mode selected by the WGSL entry shader, with each effect implemented in its own file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    Laser = 0,
    Fireball = 1,
    ThugShot = 2,
    MinibossBurst = 3,
    EnemyDamage = 4,
    PlayerDamage = 5,
    Slash = 6,
}

/// Returns the impact effect for a hostile projectile, if it should burst.
pub fn hostile_impact_effect(kind: FxKind) -> Option<FxKind> {
    match kind {
        FxKind::ThugShot | FxKind::MinibossBurst => Some(FxKind::MinibossBurst),
        _ => None,
    }
}

/// Uniform data for the animated projectile and damage-effect shader.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FxMaterial {
    #[uniform(0)]
    pub params: Vec4,
}

impl FxMaterial {
    /// Creates a shader material with a mode, time, intensity, and tint seed.
    pub fn new(kind: FxKind, time: f32, intensity: f32) -> Self {
        Self {
            params: Vec4::new(kind as i32 as f32, time, intensity, 0.0),
        }
    }
}

impl Material2d for FxMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/fx_material.wgsl".into()
    }
}

/// Registers the custom material required by the WGSL effects.
pub fn plugin() -> Material2dPlugin<FxMaterial> {
    Material2dPlugin::<FxMaterial>::default()
}

#[derive(Component)]
pub struct FxAnimation {
    pub kind: FxKind,
    pub age: f32,
    pub duration: f32,
    pub base_scale: Vec3,
}

/// Advances and removes transient shader-driven damage animations.
pub fn animate_fx(
    time: Res<Time>,
    mut commands: Commands,
    mut effects: Query<(
        Entity,
        &mut Transform,
        &mut FxAnimation,
        &Handle<FxMaterial>,
    )>,
    mut materials: ResMut<Assets<FxMaterial>>,
) {
    for (entity, mut transform, mut effect, material) in &mut effects {
        effect.age += time.delta_seconds();
        let progress = (effect.age / effect.duration).clamp(0.0, 1.0);
        transform.scale = effect.base_scale * (1.0 + progress * 0.75);
        if let Some(material) = materials.get_mut(material) {
            material.params.x = effect.kind as u32 as f32;
            material.params.y = effect.age;
            material.params.z = 1.0 - progress;
        }
        if progress >= 1.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Spawns a hostile projectile with a circular visual and finite lifetime.
pub fn spawn_hostile_projectile(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<FxMaterial>>,
    kind: FxKind,
    origin: Vec2,
    direction: Vec2,
    speed: f32,
    damage: i32,
    radius: f32,
) -> Entity {
    commands
        .spawn((
            HostileProjectile {
                velocity: direction * speed,
                damage,
                radius,
                kind,
            },
            crate::Lifetime(4.0),
            MaterialMesh2dBundle {
                mesh: meshes.add(Circle::new(radius)).into(),
                material: materials.add(FxMaterial::new(kind, 0.0, 1.0)),
                transform: Transform::from_xyz(origin.x, origin.y, 4.0),
                ..default()
            },
        ))
        .id()
}

/// Spawns a short-lived shader effect, such as an impact ring or damage burst.
pub fn spawn_fx(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<FxMaterial>>,
    kind: FxKind,
    position: Vec3,
    size: Vec2,
    duration: f32,
) {
    commands.spawn((
        FxAnimation {
            kind,
            age: 0.0,
            duration,
            base_scale: Vec3::ONE,
        },
        MaterialMesh2dBundle {
            mesh: meshes.add(Circle::new(0.5)).into(),
            material: materials.add(FxMaterial::new(kind, 0.0, 1.0)),
            transform: Transform::from_translation(position)
                .with_scale(Vec3::new(size.x, size.y, 1.0)),
            ..default()
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_projectile_preserves_direction_and_damage() {
        let projectile = HostileProjectile {
            velocity: Vec2::X * 150.0,
            damage: 8,
            radius: 12.0,
            kind: FxKind::ThugShot,
        };
        assert_eq!(projectile.velocity, Vec2::new(150.0, 0.0));
        assert_eq!(projectile.damage, 8);
        assert_eq!(projectile.radius, 12.0);
    }

    #[test]
    fn thug_and_miniboss_shots_share_the_yellow_impact_burst() {
        assert_eq!(
            hostile_impact_effect(FxKind::ThugShot),
            Some(FxKind::MinibossBurst)
        );
        assert_eq!(
            hostile_impact_effect(FxKind::MinibossBurst),
            Some(FxKind::MinibossBurst)
        );
        assert_eq!(hostile_impact_effect(FxKind::Fireball), None);
    }

    #[test]
    fn thug_projectiles_accelerate_but_stop_at_their_cap() {
        let mut projectile = HostileProjectile {
            velocity: Vec2::X * 150.0,
            damage: 8,
            radius: 12.0,
            kind: FxKind::ThugShot,
        };
        update_hostile_speed(&mut projectile, 1.0);
        assert_eq!(projectile.velocity.length(), 240.0);
        update_hostile_speed(&mut projectile, 10.0);
        assert_eq!(projectile.velocity.length(), THUG_MAX_SPEED);
    }

    #[test]
    fn miniboss_projectiles_decelerate_but_stop_at_their_floor() {
        let mut projectile = HostileProjectile {
            velocity: Vec2::X * 185.0,
            damage: 16,
            radius: 11.0,
            kind: FxKind::MinibossBurst,
        };
        update_hostile_speed(&mut projectile, 1.0);
        assert_eq!(projectile.velocity.length(), 140.0);
        update_hostile_speed(&mut projectile, 10.0);
        assert_eq!(projectile.velocity.length(), MINIBOSS_MIN_SPEED);
    }
}
