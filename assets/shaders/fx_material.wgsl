#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
}
#import "shaders/player/laserbeam.wgsl" as LaserBeam
#import "shaders/player/fireball.wgsl" as Fireball
#import "shaders/player/slash.wgsl" as Slash
#import "shaders/enemy/miniboss_burst.wgsl" as MinibossBurst
#import "shaders/enemy/enemy_damage.wgsl" as EnemyDamage
#import "shaders/player/player_damage.wgsl" as PlayerDamage

struct FxParams {
    kind: f32,
    time: f32,
    intensity: f32,
    seed: f32,
};

@group(2) @binding(0)
var<uniform> params: FxParams;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv * 2.0 - 1.0;
    if (params.kind < 0.5) { return LaserBeam::laserbeam_effect(uv, params.time, params.intensity); }
    if (params.kind < 1.5) { return Fireball::fireball_effect(uv, params.time, params.intensity); }
    if (params.kind < 2.5) { return MinibossBurst::miniboss_burst_effect_tinted(uv, params.time, params.intensity, vec3<f32>(0.78, 0.08, 1.0)); }
    if (params.kind < 3.5) { return MinibossBurst::miniboss_burst_effect(uv, params.time, params.intensity); }
    if (params.kind < 4.5) { return EnemyDamage::enemy_damage_effect(uv, params.time, params.intensity); }
    if (params.kind < 5.5) { return PlayerDamage::player_damage_effect(uv, params.time, params.intensity); }
    return Slash::slash_effect(uv, params.time, params.intensity);
}
