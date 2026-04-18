// Two-pass separable Catmull-Rom resize (Mitchell-Netravali B=0, C=0.5).
// Pass 1 (horizontal): entry point fs_horizontal
// Pass 2 (vertical):   entry point fs_vertical

struct Uniforms {
    src_size: vec2<u32>,
    dst_size: vec2<u32>,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> uni: Uniforms;

struct VertOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertOut {
    var xs = array<f32, 4>(-1.0,  1.0, -1.0,  1.0);
    var ys = array<f32, 4>(-1.0, -1.0,  1.0,  1.0);
    var out: VertOut;
    out.pos = vec4<f32>(xs[vi], ys[vi], 0.0, 1.0);
    return out;
}

// Catmull-Rom cubic kernel (support radius 2).
fn crw(x: f32) -> f32 {
    let t = abs(x);
    if t >= 2.0 { return 0.0; }
    if t < 1.0  { return 1.5*t*t*t - 2.5*t*t + 1.0; }
    return -0.5*t*t*t + 2.5*t*t - 4.0*t + 2.0;
}

@fragment
fn fs_horizontal(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let scale  = f32(uni.dst_size.x) / f32(uni.src_size.x);
    let fs     = min(1.0, scale);
    let src_cx = pos.x / scale;
    let src_y  = i32(pos.y);
    let support = 2.0 / fs;
    let lo = i32(ceil(src_cx - support));
    let hi = i32(floor(src_cx + support));
    var col = vec4<f32>(0.0);
    var ws  = 0.0;
    for (var ix = lo; ix <= hi; ix++) {
        let w  = crw((f32(ix) + 0.5 - src_cx) * fs);
        let sx = clamp(ix, 0, i32(uni.src_size.x) - 1);
        col += w * textureLoad(src, vec2<i32>(sx, src_y), 0);
        ws  += w;
    }
    if ws == 0.0 { return vec4<f32>(0.0); }
    return col / ws;
}

@fragment
fn fs_vertical(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let scale  = f32(uni.dst_size.y) / f32(uni.src_size.y);
    let fs     = min(1.0, scale);
    let src_cy = pos.y / scale;
    let src_x  = i32(pos.x);
    let support = 2.0 / fs;
    let lo = i32(ceil(src_cy - support));
    let hi = i32(floor(src_cy + support));
    var col = vec4<f32>(0.0);
    var ws  = 0.0;
    for (var iy = lo; iy <= hi; iy++) {
        let w  = crw((f32(iy) + 0.5 - src_cy) * fs);
        let sy = clamp(iy, 0, i32(uni.src_size.y) - 1);
        col += w * textureLoad(src, vec2<i32>(src_x, sy), 0);
        ws  += w;
    }
    if ws == 0.0 { return vec4<f32>(0.0); }
    return clamp(col / ws, vec4<f32>(0.0), vec4<f32>(1.0));
}
