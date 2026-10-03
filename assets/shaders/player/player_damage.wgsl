fn player_damage_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let ring_radius = 0.18 + time * 1.55;
    let outer_ring = 1.0 - smoothstep(0.0, 0.16, abs(radius - ring_radius));
    let inner_ring = 1.0 - smoothstep(0.0, 0.09, abs(radius - (ring_radius * 0.58)));
    let shards = step(0.48, fract(sin(dot(uv * 19.0 + time, vec2<f32>(41.3, 97.7))) * 18317.2));
    let radial_shards = pow(abs(cos(angle * 12.0 - time * 9.0)), 8.0) * smoothstep(1.15, 0.08, radius);
    let core = smoothstep(0.78, 0.0, radius) * (1.0 - smoothstep(0.05, 0.28, time));
    let fade = 1.0 - smoothstep(0.22, 0.46, time);
    let alpha = clamp(max(max(outer_ring, inner_ring * 0.85), max(shards * 0.62, max(radial_shards, core))) * fade * intensity, 0.0, 1.0);
    let color = mix(vec3<f32>(0.05, 0.95, 1.0), vec3<f32>(1.0, 0.08, 0.78), 0.5 + 0.5 * sin(angle * 3.0 + time * 10.0));
    return vec4<f32>(color * (1.0 + alpha * 1.6), alpha);
}
