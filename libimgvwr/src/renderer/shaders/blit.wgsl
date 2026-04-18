@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Full-screen quad as triangle strip (4 vertices, no index buffer).
    // NDC Y is +1 at top; texture UV Y is 0 at top.
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0), // bottom-left
        vec2<f32>( 1.0, -1.0), // bottom-right
        vec2<f32>(-1.0,  1.0), // top-left
        vec2<f32>( 1.0,  1.0), // top-right
    );
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0),   // bottom-left of image
        vec2<f32>(1.0, 1.0),   // bottom-right of image
        vec2<f32>(0.0, 0.0),   // top-left of image
        vec2<f32>(1.0, 0.0),   // top-right of image
    );
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vi], 0.0, 1.0);
    out.uv = uvs[vi];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(src_texture, src_sampler, in.uv);
}
