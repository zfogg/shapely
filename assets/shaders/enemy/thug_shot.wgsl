fn thug_shot_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    let radius = length(uv);
    let pulse = 0.78 + 0.22 * sin(time * 10.0);
    let alpha = smoothstep(1.0, 0.12, radius) * pulse * intensity;
    let color = mix(vec3<f32>(0.35, 0.02, 0.65), vec3<f32>(0.95, 0.15, 1.0), smoothstep(0.8, 0.0, radius));
    return vec4<f32>(color * (1.0 + alpha * 0.7), alpha);
}
