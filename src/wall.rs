//! Wall components and collision helpers for the top-down room plane.

use bevy::prelude::*;

#[derive(Component)]
/// An axis-aligned rectangular obstacle in a room.
pub struct Wall {
    pub size: Vec2,
}

/// Spawns a visible room wall and its Rapier collider.
pub fn spawn_wall(commands: &mut Commands, position: Vec2, size: Vec2, color: Color) {
    commands.spawn((
        crate::room::RoomEntity,
        Wall { size },
        SpriteBundle {
            sprite: Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            transform: Transform::from_xyz(position.x, position.y, 1.0),
            ..default()
        },
    ));
}

/// Tests a circle against an axis-aligned wall rectangle.
pub fn circle_hits_wall(position: Vec2, radius: f32, wall_center: Vec2, wall_size: Vec2) -> bool {
    let delta = position - wall_center;
    let overlap = Vec2::splat(radius) + wall_size * 0.5 - delta.abs();
    overlap.x > 0.0 && overlap.y > 0.0
}

/// Keeps the player outside wall rectangles after movement.
pub fn wall_collision(
    state: Res<crate::GameState>,
    mut player: Query<&mut Transform, With<crate::player::Player>>,
    walls: Query<(&Transform, &Wall), Without<crate::player::Player>>,
) {
    if state.mode != crate::GameMode::Playing {
        return;
    }
    let Ok(mut player) = player.get_single_mut() else {
        return;
    };
    let player_half = Vec2::splat(19.0);
    for (wall_transform, wall) in &walls {
        let delta = player.translation.truncate() - wall_transform.translation.truncate();
        let overlap = player_half + wall.size * 0.5 - delta.abs();
        if overlap.x > 0.0 && overlap.y > 0.0 {
            if overlap.x < overlap.y {
                player.translation.x += if delta.x >= 0.0 {
                    overlap.x
                } else {
                    -overlap.x
                };
            } else {
                player.translation.y += if delta.y >= 0.0 {
                    overlap.y
                } else {
                    -overlap.y
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_overlap_respects_wall_edges() {
        let center = Vec2::new(100.0, 0.0);
        let size = Vec2::new(20.0, 120.0);
        assert!(!circle_hits_wall(Vec2::new(70.0, 0.0), 6.0, center, size));
        assert!(circle_hits_wall(Vec2::new(89.0, 0.0), 6.0, center, size));
        assert!(!circle_hits_wall(Vec2::new(89.0, 70.0), 6.0, center, size));
    }
}
