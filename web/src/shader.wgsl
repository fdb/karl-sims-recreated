// Karl Sims "Evolving Virtual Creatures" style shader
// Flat per-face shading, checkered ground, depth fog

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

struct SceneUniform {
    light_dir: vec3<f32>,
    fog_near: f32,
    fog_color: vec3<f32>,
    fog_far: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> scene: SceneUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec3<f32>,
    @location(7) flags: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) @interpolate(flat) world_normal: vec3<f32>,
    @location(2) @interpolate(flat) base_color: vec3<f32>,
    @location(3) @interpolate(flat) flags: u32,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(
        instance.model_0,
        instance.model_1,
        instance.model_2,
        instance.model_3,
    );

    let world_pos = model * vec4<f32>(vertex.position, 1.0);

    // Transform normal (using upper 3x3 of model matrix)
    let normal_world = normalize(
        mat3x3<f32>(
            model[0].xyz,
            model[1].xyz,
            model[2].xyz,
        ) * vertex.normal
    );

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.world_position = world_pos.xyz;
    out.world_normal = normal_world;
    out.base_color = instance.color;
    out.flags = instance.flags;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(scene.light_dir);
    let normal = normalize(in.world_normal);
    let ndotl = dot(normal, light_dir);

    var color: vec3<f32>;

    let is_ground = (in.flags & 1u) != 0u;

    if is_ground {
        // Checkered ground plane
        let cx = floor(in.world_position.x / 2.0);
        let cz = floor(in.world_position.z / 2.0);
        let checker = ((i32(cx) + i32(cz)) % 2 + 2) % 2; // ensure positive modulo
        let light_tile = vec3<f32>(0.48, 0.55, 0.58);
        let dark_tile = vec3<f32>(0.35, 0.42, 0.46);
        let base = select(dark_tile, light_tile, checker == 0);
        // Subtle lighting on ground
        let lighting = 0.7 + max(ndotl, 0.0) * 0.3;
        color = base * lighting;
    } else {
        // Creature box: Karl Sims flat shading
        // Shadow faces get olive/yellow-green ambient tint
        let shadow_ambient = vec3<f32>(0.38, 0.40, 0.34);
        // Lit faces add cream diffuse
        let diffuse = in.base_color * max(ndotl, 0.0) * 0.65;
        color = shadow_ambient + diffuse;
    }

    // Depth fog
    let dist = distance(camera.camera_pos, in.world_position);
    let fog_factor = clamp((dist - scene.fog_near) / (scene.fog_far - scene.fog_near), 0.0, 1.0);
    color = mix(color, scene.fog_color, fog_factor);

    return vec4<f32>(color, 1.0);
}
