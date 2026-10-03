fn slash_effect(uv: vec2<f32>, time: f32, intensity: f32) -> vec4<f32> {
    // The transform spins clockwise. Flip the blade to the opposite side on
    // each half-turn so the leading edge follows the direction of the swing.
    let blade_side = select(-1.0, 1.0, sin(time * 12.0) >= 0.0);
    let signed_distance = (uv.y - uv.x * 0.28) * blade_side;

    // step() gives the cutting edge a deliberately hard silhouette; only the
    // back edge and the tip feather out slightly so it still reads as energy.
    let hard_cutting_edge = step(0.0, signed_distance);
    let back_edge = 1.0 - smoothstep(0.18, 0.25, signed_distance);
    let blade_length = smoothstep(1.05, 0.72, abs(uv.x));
    let blade = hard_cutting_edge * back_edge * blade_length;
    let edge_glint = step(0.0, signed_distance) * (1.0 - smoothstep(0.025, 0.06, signed_distance));
    let alpha = (blade * 0.9 + edge_glint * 0.8) * (0.82 + 0.18 * sin(time * 18.0)) * intensity;
    let color = mix(vec3<f32>(1.0, 0.08, 0.42), vec3<f32>(1.0, 0.88, 0.98), edge_glint);
    return vec4<f32>(color * (1.0 + alpha), alpha);
}
