@group(0) @binding(0) var atlas: texture_2d_array<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var font: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) data: u32,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) mode: u32,
    @location(3) @interpolate(flat) layer: u32,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.clip = vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.mode = in.data & 0xffu;
    out.layer = in.data >> 8u;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    var c = in.color;
    if (in.mode == 1u) {
        let t = textureSample(atlas, samp, in.uv, in.layer);
        c = c * t;
    } else if (in.mode == 2u) {
        let t = textureSample(font, samp, in.uv);
        c = vec4<f32>(c.rgb, c.a * t.a);
    }
    if (c.a < 0.01) {
        discard;
    }
    return c;
}
