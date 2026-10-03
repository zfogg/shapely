//! Room geometry constants and deterministic room identity helpers.

use bevy::prelude::*;

/// World-space dimensions of every room.
pub const ROOM_SIZE: Vec2 = Vec2::new(1120.0, 620.0);

/// Produces a stable pseudo-random seed for a room coordinate.
pub fn seed(room: IVec2) -> u32 {
    let x = room.x as u32;
    let y = room.y as u32;
    x.wrapping_mul(0x9E37_79B9).rotate_left(13) ^ y.wrapping_mul(0x85EB_CA6B).rotate_right(7) ^ 0xC0FF_EE11
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_seed_is_deterministic_and_varies_by_room() {
        assert_eq!(seed(IVec2::ZERO), seed(IVec2::ZERO));
        assert_ne!(seed(IVec2::ZERO), seed(IVec2::new(1, 0)));
    }
}
