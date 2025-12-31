#import bevy_pbr::{
    mesh_functions,
    pbr_functions::prepare_world_normal,
    view_transformations::position_world_to_clip
}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) local_normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(2) color: vec4<f32>,
};

// See https://www.reedbeta.com/blog/quick-and-easy-gpu-random-numbers-in-d3d11/
fn hash(seed: u32) -> u32 {
    var value = seed;
    value = (value ^ 61u) ^ (value >> 16u);
    value *= 9u;
    value = value ^ (value >> 4u);
    value *= 0x27d4eb2du;
    value = value ^ (value >> 15u);
    return value;
}

fn random_f32(seed: u32) -> f32 {
    let hashed = hash(seed);
    return f32(hashed & 0xFFFFFFFu) / f32(0x10000000u);
}

fn hsv_to_rgba(hue: f32) -> vec4<f32> {
    let h = hue * 6.0; // Scale hue to 0..6
    let i = u32(h); // Integer part of h (0, 1, 2, 3, 4, or 5)
    let f = h - f32(i); // Fractional part of h
    let p = 0.0; // Min saturation and value are always 0
    let q = 1.0 - f; // Decreasing value
    let t = f; // Increasing value

    var r: f32;
    var g: f32;
    var b: f32;

    if (i == 0u) {
        r = 1.0;
        g = t;
        b = p;
    } else if (i == 1u) {
        r = q;
        g = 1.0;
        b = p;
    } else if (i == 2u) {
        r = p;
        g = 1.0;
        b = t;
    } else if (i == 3u) {
        r = p;
        g = q;
        b = 1.0;
    } else if (i == 4u) {
        r = t;
        g = p;
        b = 1.0;
    } else {
        r = 1.0;
        g = p;
        b = q;
    }

    return vec4<f32>(r, g, b, 1.0); // Return as RGBA
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let tag = mesh_functions::get_tag(vertex.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

    // compute world position and normal info
    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.clip_position = position_world_to_clip(world_position.xyz);

    // generate colour from mesh instance
    let frac = clamp(dot(vertex.local_normal, vec3<f32>(0.0, 1.0, 0.0)), 0.5, 1.0);
    out.color = hsv_to_rgba(random_f32(tag)) * frac;

    return out;
}

@fragment
fn fragment(
    mesh: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    return mesh.color;
}
