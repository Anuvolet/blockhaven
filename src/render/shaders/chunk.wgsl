struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    fog_color: vec4<f32>,
    // fog_start, fog_end, sun_level (0..1), time
    params: vec4<f32>,
    tints: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) data: u32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) color: vec3<f32>,
    @location(3) fog: f32,
    @location(4) @interpolate(flat) tint: u32,
};

const FACE_SHADE: array<f32, 6> = array<f32, 6>(0.62, 0.62, 0.5, 1.0, 0.8, 0.8);

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.clip = g.view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    out.layer = in.data & 0xfffu;
    let normal = (in.data >> 12u) & 7u;
    out.tint = (in.data >> 15u) & 7u;
    let ao = f32((in.data >> 18u) & 3u);
    let sky = f32((in.data >> 20u) & 15u);
    let blk = f32((in.data >> 24u) & 15u);
    let sun = sky * g.params.z;
    let l = max(sun, blk) / 15.0;
    let bright = 0.035 + 0.965 * pow(l, 1.6);
    let warm = clamp((blk - sun) / 15.0, 0.0, 1.0) * (blk / 15.0);
    var col = vec3<f32>(bright) * mix(vec3<f32>(1.0), vec3<f32>(1.0, 0.86, 0.66), warm);
    // night tint: sky-lit surfaces go slightly blue
    let night = 1.0 - g.params.z;
    col = col * mix(vec3<f32>(1.0), vec3<f32>(0.78, 0.84, 1.0), night * (sun / max(max(sun, blk), 0.001)));
    let ao_f = 0.45 + 0.55 * (ao / 3.0);
    var shade = 1.0;
    switch normal {
        case 0u, 1u: { shade = 0.62; }
        case 2u: { shade = 0.5; }
        case 3u: { shade = 1.0; }
        default: { shade = 0.8; }
    }
    out.color = col * ao_f * shade;
    let d = distance(in.pos, g.cam_pos.xyz);
    out.fog = clamp((d - g.params.x) / max(g.params.y - g.params.x, 1.0), 0.0, 1.0);
    return out;
}

@fragment
fn fs_opaque(in: VOut) -> @location(0) vec4<f32> {
    var tex = textureSample(atlas, samp, in.uv, in.layer);
    if (tex.a < 0.5) {
        discard;
    }
    var rgb = tex.rgb;
    if (in.tint != 0u) {
        rgb = rgb * g.tints[in.tint].rgb;
    }
    rgb = rgb * in.color;
    rgb = mix(rgb, g.fog_color.rgb, in.fog);
    return vec4<f32>(rgb, 1.0);
}

@fragment
fn fs_translucent(in: VOut) -> @location(0) vec4<f32> {
    var tex = textureSample(atlas, samp, in.uv, in.layer);
    if (tex.a < 0.02) {
        discard;
    }
    var rgb = tex.rgb;
    if (in.tint != 0u) {
        rgb = rgb * g.tints[in.tint].rgb;
    }
    rgb = rgb * in.color;
    rgb = mix(rgb, g.fog_color.rgb, in.fog);
    return vec4<f32>(rgb, tex.a);
}
