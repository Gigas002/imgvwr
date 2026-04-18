// Pixel-perfect 90 / 180 / 270° rotation via textureLoad (no sampler).
// Matches image-rs rotate90 / rotate180 / rotate270 conventions.

struct Uniforms {
    src_w:    u32,
    src_h:    u32,
    rotation: u32,  // 0=0°  1=90°  2=180°  3=270°
    _pad:     u32,
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

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = i32(pos.x);
    let py = i32(pos.y);
    let sw = i32(uni.src_w);
    let sh = i32(uni.src_h);
    var sx: i32;
    var sy: i32;
    // image-rs rotate90 maps src(x,y) → out(sh-1-y, x); output size (sh, sw).
    // image-rs rotate270 maps src(x,y) → out(y, sw-1-x); output size (sh, sw).
    if uni.rotation == 1u {          // 90°  — output (sh, sw)
        sx = py;
        sy = sh - 1 - px;
    } else if uni.rotation == 2u {   // 180° — output (sw, sh)
        sx = sw - 1 - px;
        sy = sh - 1 - py;
    } else if uni.rotation == 3u {   // 270° — output (sh, sw)
        sx = sw - 1 - py;
        sy = px;
    } else {
        sx = px;
        sy = py;
    }
    return textureLoad(src, vec2<i32>(sx, sy), 0);
}
