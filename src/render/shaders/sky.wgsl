struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    // sun_level (0..1), time_of_day (0..1), star_seed, unused
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: SkyUniform;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    var pos = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var out: VOut;
    out.clip = vec4<f32>(pos[vi], 0.99999, 1.0);
    out.ndc = pos[vi];
    return out;
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(443.897, 441.423, 437.195));
    q = q + dot(q, q.yzx + 19.19);
    return fract((q.x + q.y) * q.z);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let p = u.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(p.xyz / p.w);
    let up = dir.y;
    let sun_level = u.params.x;
    var col: vec3<f32>;
    if (up >= 0.0) {
        col = mix(u.horizon.rgb, u.zenith.rgb, pow(clamp(up, 0.0, 1.0), 0.55));
    } else {
        let below = mix(u.horizon.rgb, u.zenith.rgb * 0.5, clamp(-up * 3.0, 0.0, 1.0));
        col = below;
    }
    // sun
    let sd = normalize(u.sun_dir.xyz);
    let cs = dot(dir, sd);
    let sun_disc = smoothstep(0.9985, 0.9992, cs);
    let sun_glow = pow(max(cs, 0.0), 24.0) * 0.35 * sun_level;
    let horizon_glow = pow(max(cs, 0.0), 4.0) * 0.25 * (1.0 - abs(up)) * smoothstep(-0.2, 0.3, sd.y) * (1.0 - smoothstep(0.3, 0.7, sd.y));
    col = col + vec3<f32>(1.0, 0.85, 0.6) * (sun_glow + horizon_glow);
    col = mix(col, vec3<f32>(1.0, 0.98, 0.9), sun_disc * smoothstep(-0.1, 0.05, sd.y));
    // moon (opposite the sun)
    let md = -sd;
    let cm = dot(dir, md);
    let moon_disc = smoothstep(0.9990, 0.9995, cm);
    let moon_dark = smoothstep(0.9985, 0.9992, dot(dir, normalize(md + vec3<f32>(0.012, 0.01, 0.0))));
    let moon = clamp(moon_disc - moon_dark * 0.85, 0.0, 1.0);
    col = mix(col, vec3<f32>(0.9, 0.92, 1.0), moon * smoothstep(-0.1, 0.05, md.y));
    // stars
    let night = 1.0 - smoothstep(0.25, 0.6, sun_level);
    if (night > 0.0 && up > 0.0) {
        let cell = floor(dir * 110.0 + u.params.z);
        let h = hash3(cell);
        let local = fract(dir * 110.0 + u.params.z) - 0.5;
        let d = length(local);
        if (h > 0.972 && d < 0.3) {
            let tw = 0.75 + 0.25 * sin(u.params.y * 400.0 + h * 60.0);
            let b = (1.0 - d / 0.3) * night * tw * clamp(up * 4.0, 0.0, 1.0) * 1.3;
            col = col + vec3<f32>(b);
        }
    }
    return vec4<f32>(col, 1.0);
}
