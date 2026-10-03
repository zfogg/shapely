//! Miniboss balance constants: radial attacks and minion summons.

/// Miniboss health pool.
pub const HEALTH: i32 = 80;
/// Miniboss collision radius.
pub const RADIUS: f32 = 48.0;
/// Miniboss movement speed.
pub const MOVE_SPEED: f32 = 10.0;
/// Miniboss contact damage.
pub const CONTACT_DAMAGE: i32 = 14;
/// Miniboss radial attack cadence.
pub const ATTACK_COOLDOWN: f32 = 3.0;
/// Number of projectiles in a radial burst.
pub const RADIAL_PROJECTILES: usize = 8;
/// Damage of each miniboss radial projectile.
pub const PROJECTILE_DAMAGE: i32 = 16;
/// Time between minion summons.
pub const SUMMON_COOLDOWN: f32 = 6.0;
