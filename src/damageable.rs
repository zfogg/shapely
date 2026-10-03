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
    fn is_alive(&self) -> bool { self.health() > 0 }
}
