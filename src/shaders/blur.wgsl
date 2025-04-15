// Vertex shader for a fullscreen triangle
@vertex
fn vs_main(@builtin(vertex_index) vert_id: u32) -> @builtin(position) vec4<f32> {
    // Create a fullscreen triangle with just the vertex id
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    
    return vec4<f32>(positions[vert_id], 0.0, 1.0);
}

// Gaussian blur fragment shader
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> direction: vec2<f32>; // (1,0) or (0,1)
@group(0) @binding(3) var<uniform> adaptive_scaling: f32;
@group(0) @binding(4) var<uniform> max_radius: f32;

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(tex));
    let tex_coord = pos.xy / tex_size;
    
    // Get the center pixel to determine base brightness
    let center_pixel = textureSample(tex, tex_sampler, tex_coord);
    
    // Base brightness is stored in alpha from brightness pass
    // For vertical pass, we need to estimate from color intensity
    let base_brightness = max(center_pixel.a, length(center_pixel.rgb) * 0.5);
    
    // Dynamic blur parameters based on brightness
    let base_radius = 4.5;
    
    // Scale radius with brightness (non-linear scaling for more dramatic effect)
    let brightness_factor = pow(base_brightness, adaptive_scaling); // Non-linear scaling
    let blur_radius = mix(base_radius, max_radius, brightness_factor);
    
    let sigma = blur_radius / 3.0;
    
    // Gaussian blur calculation
    var result = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var weight_sum = 0.0;
    
    // Sample multiple pixels along the blur direction
    for (var i = -blur_radius; i <= blur_radius; i += 1.0) {
        let offset = direction * i / tex_size;
        let sample_pos = tex_coord + offset;
        
        // Calculate Gaussian weight
        let weight = exp(-(i * i) / (2.0 * sigma * sigma));
        
        // Sample and accumulate
        let sample = textureSample(tex, tex_sampler, sample_pos);
        result += sample * weight;
        weight_sum += weight;
    }
    
    // Normalize by weight sum
    // Preserve our brightness information in alpha for the next stage
    var normalized = result / weight_sum;
    normalized.a = base_brightness; // Pass brightness information to next stage
    return normalized;
}