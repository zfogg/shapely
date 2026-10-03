fn fireball_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    let radius = length(uv);
    let flicker = 0.78 + 0.22 * sin(time * 12.0 + uv.x * 8.0 + uv.y * 5.0);
    let shell = smoothstep(1.0, 0.08, radius) * flicker;
    let core = smoothstep(0.55, 0.0, radius);
    let color = mix(vec3<f32>(1.0, 0.06, 0.01), vec3<f32>(1.0, 0.92, 0.24), core);
    return vec4<f32>(color * (1.0 + shell * 0.65), shell * intensity);
}
