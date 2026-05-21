#import bevy_pbr::forward_io::VertexOutput

struct ProceduralMaterial {
    base_color: vec4<f32>,
    noise_scale: f32,
    noise_strength: f32,
    color_variation: vec4<f32>,  // rgb = variation amount, a = unused
    roughness: f32,
    _padding: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: ProceduralMaterial;

// Simple hash-based noise (no texture lookups needed)
fn hash3(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise with smooth interpolation
fn value_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Smooth interpolation curve
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash3(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));

    let x0 = mix(n000, n100, u.x);
    let x1 = mix(n010, n110, u.x);
    let x2 = mix(n001, n101, u.x);
    let x3 = mix(n011, n111, u.x);

    let y0 = mix(x0, x1, u.y);
    let y1 = mix(x2, x3, u.y);

    return mix(y0, y1, u.z);
}

// fBM — layered noise
fn fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 1.0;
    var frequency = 1.0;
    var max_amp = 0.0;
    var pos = p;

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + value_noise(pos * frequency) * amplitude;
        max_amp = max_amp + amplitude;
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }
    return value / max_amp;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let normal = normalize(in.world_normal);

    // Evaluate noise at world position
    let noise_pos = world_pos * material.noise_scale;
    let noise_val = fbm(noise_pos, 4) * 2.0 - 1.0; // -1 to 1

    // Apply color variation based on noise
    let variation = material.color_variation.rgb * noise_val * material.noise_strength;
    var color = material.base_color.rgb + variation;

    // Simple directional lighting (sun from upper left)
    let light_dir = normalize(vec3<f32>(0.4, 0.8, 0.3));
    let ndotl = max(dot(normal, light_dir), 0.0);
    let ambient = 0.15;
    let diffuse = ndotl;

    // Specular (Blinn-Phong, view = straight down for top-down)
    let view_dir = normalize(vec3<f32>(0.3, 0.8, 0.3));
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0 / material.roughness) * (1.0 - material.roughness) * 0.3;

    let lit = color * (ambient + diffuse) + vec3<f32>(spec);

    return vec4<f32>(lit, material.base_color.a);
}
