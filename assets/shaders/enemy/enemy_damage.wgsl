fn enemy_damage_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let noise = fract(sin(dot(uv * 22.0 + time, vec2<f32>(127.1, 311.7))) * 43758.5453123);
    let ring_radius = 0.22 + time * 1.8;
    let ring = 1.0 - smoothstep(0.0, 0.12, abs(radius - ring_radius));
    let core = smoothstep(0.72, 0.0, radius);
    let spikes = pow(abs(cos(angle * 10.0 + time * 8.0)), 14.0) * smoothstep(1.2, 0.15, radius);
    let shards = step(0.42, noise) * smoothstep(1.15, 0.05, radius);
    let fade = 1.0 - smoothstep(0.18, 0.38, time);
    let alpha = clamp(max(max(ring, spikes * 1.4), max(shards * 0.95, core * 0.7)) * fade * intensity, 0.0, 1.0);
    let color = mix(vec3<f32>(1.0, 0.01, 0.02), vec3<f32>(1.0, 0.9, 0.25), core);
    return vec4<f32>(color * (1.0 + alpha * 1.4), alpha);
}
