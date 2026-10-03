fn miniboss_burst_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    return miniboss_burst_effect_tinted(uv, time, intensity, vec3<f32>(1.0, 0.68, 0.06));
}

fn miniboss_burst_effect_tinted(uv: vec2<f32>, time: f32, intensity: f32, tint: vec3<f32>) -> vec4<f32> {
    let radius = length(uv);
    let angle = atan2(uv.y, uv.x);
    let spokes = pow(abs(cos(angle * 8.0 + time * 3.0)), 10.0);
    let alpha = smoothstep(1.0, 0.08, radius) * (0.42 + 0.8 * spokes) * intensity;
    return vec4<f32>(tint * (1.0 + alpha), alpha);
}
