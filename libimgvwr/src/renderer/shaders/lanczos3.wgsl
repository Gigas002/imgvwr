// Two-pass separable Lanczos3 resize.
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

// Lanczos kernel with window a=3.
fn lw(x: f32) -> f32 {
    let a = 3.0;
    let t = abs(x);
    if t >= a    { return 0.0; }
    if t < 0.001 { return 1.0; }
    let pt = 3.14159265358979 * t;
    return (sin(pt) / pt) * (sin(pt / a) / (pt / a));
}

@fragment
fn fs_horizontal(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let scale = f32(uni.dst_size.x) / f32(uni.src_size.x);
    // Filter scale: compress kernel for downscaling to avoid aliasing.
    let fs    = min(1.0, scale);
    let src_cx = pos.x / scale;
    let src_y  = i32(pos.y);
    let support = 3.0 / fs;
    let lo = i32(ceil(src_cx - support));
    let hi = i32(floor(src_cx + support));
    var col = vec4<f32>(0.0);
    var ws  = 0.0;
    for (var ix = lo; ix <= hi; ix++) {
        let w  = lw((f32(ix) + 0.5 - src_cx) * fs);
        let sx = clamp(ix, 0, i32(uni.src_size.x) - 1);
        col += w * textureLoad(src, vec2<i32>(sx, src_y), 0);
        ws  += w;
    }
    if ws == 0.0 { return vec4<f32>(0.0); }
    // No clamp: Rgba16Float intermediate preserves negative Lanczos lobes.
    return col / ws;
}

@fragment
fn fs_vertical(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let scale  = f32(uni.dst_size.y) / f32(uni.src_size.y);
    let fs     = min(1.0, scale);
    let src_cy = pos.y / scale;
    let src_x  = i32(pos.x);
    let support = 3.0 / fs;
    let lo = i32(ceil(src_cy - support));
    let hi = i32(floor(src_cy + support));
    var col = vec4<f32>(0.0);
    var ws  = 0.0;
    for (var iy = lo; iy <= hi; iy++) {
        let w  = lw((f32(iy) + 0.5 - src_cy) * fs);
        let sy = clamp(iy, 0, i32(uni.src_size.y) - 1);
        col += w * textureLoad(src, vec2<i32>(src_x, sy), 0);
        ws  += w;
    }
    if ws == 0.0 { return vec4<f32>(0.0); }
    // Clamp on final write to Rgba8Unorm.
    return clamp(col / ws, vec4<f32>(0.0), vec4<f32>(1.0));
}
