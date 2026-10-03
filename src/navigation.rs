//! Grid-based navigation for room layouts and enemy A* pursuit.
//!
//! Rooms are rasterized into cells. Wall cells and cells occupied by other
//! enemies are excluded from paths, while diagonal movement is allowed when
//! it does not cut through a blocked corner.

use bevy::prelude::*;
use pathfinding::prelude::astar;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// A coordinate in a room's navigation grid.
pub struct GridPos { pub x: i32, pub y: i32 }

#[derive(Resource)]
/// Navigation state for the currently loaded room.
pub struct RoomNavGrid {
    pub room: IVec2,
    pub width: i32,
    pub height: i32,
    pub cell_size: f32,
    pub origin: Vec2,
    pub blocked: Vec<bool>,
}

impl Default for RoomNavGrid {
    fn default() -> Self { Self { room: IVec2::ZERO, width: 28, height: 16, cell_size: 40.0, origin: Vec2::new(-560.0, -320.0), blocked: vec![false; 28 * 16] } }
}

impl RoomNavGrid {
    /// Converts a cell to its backing-array index when it is in bounds.
    pub fn index(&self, cell: GridPos) -> Option<usize> { if self.in_bounds(cell) { Some((cell.y * self.width + cell.x) as usize) } else { None } }
    pub fn in_bounds(&self, cell: GridPos) -> bool { cell.x >= 0 && cell.y >= 0 && cell.x < self.width && cell.y < self.height }
    pub fn is_open(&self, cell: GridPos) -> bool { self.index(cell).map(|index| !self.blocked[index]).unwrap_or(false) }
    pub fn world_to_cell(&self, position: Vec2) -> GridPos { GridPos { x: ((position.x - self.origin.x) / self.cell_size).floor() as i32, y: ((position.y - self.origin.y) / self.cell_size).floor() as i32 } }
    pub fn cell_to_world(&self, cell: GridPos) -> Vec2 { self.origin + Vec2::new((cell.x as f32 + 0.5) * self.cell_size, (cell.y as f32 + 0.5) * self.cell_size) }
    pub fn nearest_open(&self, start: GridPos) -> Option<GridPos> { for radius in 0..self.width.max(self.height) { for y in (start.y - radius)..=(start.y + radius) { for x in (start.x - radius)..=(start.x + radius) { let cell = GridPos { x, y }; if self.is_open(cell) { return Some(cell); } } } } None }
    pub fn neighbors(&self, cell: GridPos) -> Vec<(GridPos, usize)> {
        let mut result = Vec::new();
        for (dx, dy, cost) in [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (-1, 1, 14), (1, -1, 14), (-1, -1, 14)] {
            let candidate = GridPos { x: cell.x + dx, y: cell.y + dy };
            if !self.is_open(candidate) { continue; }
            if dx != 0 && dy != 0 && (!self.is_open(GridPos { x: cell.x + dx, y: cell.y }) || !self.is_open(GridPos { x: cell.x, y: cell.y + dy })) { continue; }
            result.push((candidate, cost));
        }
        result
    }
    /// Rebuilds blocked cells from the room's wall rectangles.
    pub fn rebuild(&mut self, room: IVec2, walls: &[(Vec2, Vec2)]) {
        self.room = room;
        self.blocked.fill(false);
        let clearance = 36.0;
        for y in 0..self.height { for x in 0..self.width { let center = self.cell_to_world(GridPos { x, y }); self.blocked[(y * self.width + x) as usize] = walls.iter().any(|(wall_center, wall_size)| { let delta = center - *wall_center; let overlap = Vec2::splat(clearance) + *wall_size * 0.5 - delta.abs(); overlap.x > 0.0 && overlap.y > 0.0 }); } }
    }
}

/// Finds an enemy path while routing around walls and other enemies.
pub fn find_enemy_path(nav: &RoomNavGrid, start: GridPos, goal: GridPos, entity: Entity, occupied_enemies: &[(Entity, Vec2, f32)]) -> Option<Vec<GridPos>> {
    let successors = |cell: &GridPos| nav.neighbors(*cell).into_iter().filter(|(next, _)| {
        if *next == goal || *next == start { return true; }
        let center = nav.cell_to_world(*next);
        !occupied_enemies.iter().any(|(other_entity, other_position, other_radius)| *other_entity != entity && center.distance(*other_position) < *other_radius + 28.0)
    }).collect::<Vec<_>>();
    astar(&start, successors, |cell| { let dx = (cell.x - goal.x).unsigned_abs() as usize; let dy = (cell.y - goal.y).unsigned_abs() as usize; dx.max(dy) * 10 + dx.min(dy) * 4 }, |cell| *cell == goal).map(|(path, _)| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_cell_round_trip_and_blocked_cells() {
        let mut nav = RoomNavGrid::default();
        let cell = GridPos { x: 10, y: 7 };
        assert_eq!(nav.world_to_cell(nav.cell_to_world(cell)), cell);
        nav.blocked[(7 * nav.width + 10) as usize] = true;
        assert!(!nav.is_open(cell));
        assert!(nav.nearest_open(cell).is_some());
    }

    #[test]
    fn diagonal_path_avoids_occupied_cells() {
        let nav = RoomNavGrid::default();
        let start = GridPos { x: 2, y: 2 };
        let goal = GridPos { x: 10, y: 8 };
        let occupied = [(Entity::from_raw(99), nav.cell_to_world(GridPos { x: 6, y: 5 }), 23.0)];
        let path = find_enemy_path(&nav, start, goal, Entity::from_raw(1), &occupied).unwrap();
        assert!(path.windows(2).any(|step| (step[1].x - step[0].x).abs() == 1 && (step[1].y - step[0].y).abs() == 1));
        assert!(!path.contains(&GridPos { x: 6, y: 5 }));
    }
}
