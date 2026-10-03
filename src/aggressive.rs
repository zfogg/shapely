//! Timers, transient lifetimes, and other continuously advancing gameplay state.

use bevy::prelude::*;

/// Ticks combat, potion, damage, and room-notice timers while appropriate.
pub(crate) fn update_cooldowns(time: Res<Time>, mut state: ResMut<crate::GameState>) {
    if state.mode == crate::GameMode::Playing {
        for cooldown in &mut state.cooldowns {
            *cooldown = (*cooldown - time.delta_seconds()).max(0.0);
        }
        state.attack_timer = (state.attack_timer - time.delta_seconds()).max(0.0);
        state.speed_boost = (state.speed_boost - time.delta_seconds()).max(0.0);
        state.attack_speed_boost = (state.attack_speed_boost - time.delta_seconds()).max(0.0);
        state.damage_cooldown = (state.damage_cooldown - time.delta_seconds()).max(0.0);
    }
    state.room_notice = (state.room_notice - time.delta_seconds()).max(0.0);
}

/// Removes entities whose gameplay lifetime has expired.
pub(crate) fn lifetime_cleanup(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<crate::GameState>,
    mut entities: Query<(Entity, &mut crate::Lifetime)>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    for (entity, mut lifetime) in &mut entities {
        lifetime.0 -= time.delta_seconds();
        if lifetime.0 <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
