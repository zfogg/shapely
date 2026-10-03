fn laserbeam_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    let beam = exp(-abs(uv.y) * 13.0);
    let scan = 0.65 + 0.35 * sin(uv.x * 34.0 - time * 20.0);
    let alpha = beam * scan * intensity;
    let color = vec3<f32>(0.08, 0.82, 1.0) * (1.0 + alpha * 0.85);
    return vec4<f32>(color, alpha);
}
