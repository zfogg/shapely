# Shapely Agent Instructions

## Project

Shapely is a colorful top-down action game written in Rust 2021. Bevy 0.14
provides the ECS, renderer, sprites, text, UI, windowing, assets, and audio.
`bevy_rapier2d` provides 2D physics support, and `pathfinding` provides A* enemy
navigation through room cells. `rstest` is used for focused test cases.

The game has nine persistent rooms, a player with three mouse-aimed abilities,
enemy archetypes, pickups, inventory, upgrades, menus, audio cues, and native /
WASM targets. It is top-down: the screen plane is the floor, so do not add
gravity-based platformer behavior.

## Code organization

- `src/main.rs`: app wiring, global state, room transitions, HUD, and schedules.
- `src/player.rs`: player state, movement, attack profiles, and abilities.
- `src/enemy/`: shared enemy behavior plus `minion.rs`, `thug.rs`, and `miniboss.rs`.
- `src/room.rs`, `src/wall.rs`, and `src/navigation.rs`: room geometry, collisions, and A* grids.
- `src/projectable.rs`: projectiles, custom materials, and transient visual effects.
- `src/damageable.rs`: shared damage behavior and invariants.

Keep code in modules and files that make sense to an English-speaking human.
Prefer small, cohesive systems and data types over growing one giant function.
Keep room state room-scoped; do not regenerate enemies, projectiles, pickups, or
other gameplay state merely because the player crossed a doorway.

## Rendering and effects

Custom WGSL lives under `assets/shaders/`. Player shaders are in
`assets/shaders/player/`; enemy shaders are in `assets/shaders/enemy/`.
`fx_material.wgsl` is the small Bevy `Material2d` dispatcher. Keep actual effect
logic in the appropriate per-effect WGSL file, and validate shader changes by
launching the native game because Rust compilation does not compile every shader
pipeline. Preserve WASM-compatible shader and asset paths.

## Working rules

- Keep `cargo run` working immediately after changes; build it before handoff so
  the user can run it without discovering avoidable compile errors.
- Run `cargo test` before every commit and do not commit with failing tests.
- For platform-sensitive changes, run `cargo check --target wasm32-unknown-unknown` too.
- Use idiomatic, strongly typed, ownership-aware, ECS-friendly Rust: be very
  “rusty.” Avoid unnecessary clones, hidden global state, panics, and speculative
  abstractions.
- Preserve native rendering, audio, UI, physics, and browser support when editing
  adjacent systems.
- Use `rg` / `rg --files` for searching; use `dust` for directory-size inspection.
  Do not use `grep` or `du`.
