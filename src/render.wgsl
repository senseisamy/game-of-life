struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct CameraUniform {
    offset: vec2<f32>,
    scale: f32
};

@group(1) @binding(0) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // full screen triangle
    let clip_positions = array<vec2<f32>, 3> (
        vec2<f32>(-1.0,  1.0), // top left
        vec2<f32>(-1.0, -3.0), // far bottom left
        vec2<f32>( 3.0,  1.0), // far top right
    );

    let uvs = array<vec2<f32>, 3> (
        vec2<f32>(0.0, 0.0), // top left
        vec2<f32>(0.0, 2.0), // far bottom left
        vec2<f32>(2.0, 0.0), // far top right
    );
    // let clip_positions = array<vec2<f32>, 3> (
    //     vec2<f32>(0.0, 0.8), // top
    //     vec2<f32>(-0.5, -0.5), // left
    //     vec2<f32>(0.5, -0.5), // right
    // );

    out.position = vec4<f32>(clip_positions[vertex_index] * vec2<f32>(camera.scale) + camera.offset , 0.0, 1.0);
    out.uv = uvs[vertex_index];

    return out;
}

@group(0) @binding(0)  var t_grid: texture_2d<f32>;
@group(0) @binding(1) var s_grid: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_grid, s_grid, in.uv);
}

// @fragment
// fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
//     return vec4<f32>(1.0, 0.0, 0.0, 1.0);
// }