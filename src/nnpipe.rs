// src/post/post_processing.rs
//
// Texture rendering and post-processing

use nannou::prelude::*;
use nannou::wgpu;

#[allow(dead_code)]
pub struct Nnpipe {
    // Textures for the pipeline
    pub scene_texture: wgpu::Texture,
    pub brightness_texture: wgpu::Texture,
    pub blur_h_texture: wgpu::Texture,
    pub blur_v_texture: wgpu::Texture,
    pub composite_texture: wgpu::Texture,

    // Texture views
    pub scene_view: wgpu::TextureView,
    pub brightness_view: wgpu::TextureView,
    pub blur_h_view: wgpu::TextureView,
    pub blur_v_view: wgpu::TextureView,
    pub composite_view: wgpu::TextureView,

    // Render pipelines for each pass
    brightness_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,

    // Adaptive bloom
    pub adaptive_blur_scaling: f32,
    pub max_blur_radius: f32,
    pub intensity_curve: f32,

    // Pipeline parameters
    pub brightness_threshold: f32,
    pub bloom_intensity: f32,

    // Shader bind groups
    pub brightness_bind_group: wgpu::BindGroup,
    pub blur_h_bind_group: wgpu::BindGroup,
    pub blur_v_bind_group: wgpu::BindGroup,
    pub composite_bind_group: wgpu::BindGroup,

    // Sampler for texture sampling
    sampler: wgpu::Sampler,

    // Uniform buffers for parameters
    threshold_buffer: wgpu::Buffer,
    blur_h_buffer: wgpu::Buffer,
    blur_v_buffer: wgpu::Buffer,
    intensity_buffer: wgpu::Buffer,

    adaptive_scaling_buffer: wgpu::Buffer,
    max_radius_buffer: wgpu::Buffer,
    intensity_curve_buffer: wgpu::Buffer,
}

impl Nnpipe {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, samples: u32) -> Self {
        // Create textures
        let scene_texture = create_render_texture(device, width, height, samples);
        let brightness_texture = create_render_texture(device, width, height, 1);
        let blur_h_texture = create_render_texture(device, width, height, 1);
        let blur_v_texture = create_render_texture(device, width, height, 1);
        let composite_texture = create_render_texture(device, width, height, 1);

        // Create texture views
        let scene_view = scene_texture.view().build();
        let brightness_view = brightness_texture.view().build();
        let blur_h_view = blur_h_texture.view().build();
        let blur_v_view = blur_v_texture.view().build();
        let composite_view = composite_texture.view().build();

        // Create a sampler for texture sampling
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create uniform buffers
        let brightness_threshold = 0.55f32;
        let threshold_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Threshold Buffer"),
            contents: bytemuck::cast_slice(&[brightness_threshold]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Horizontal blur direction (1.0, 0.0)
        let blur_h_direction = [1.0f32, 0.0f32];
        let blur_h_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Horizontal Blur Buffer"),
            contents: bytemuck::cast_slice(&blur_h_direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Vertical blur direction (0.0, 1.0)
        let blur_v_direction = [0.0f32, 0.7f32];
        let blur_v_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertical Blur Buffer"),
            contents: bytemuck::cast_slice(&blur_v_direction),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bloom intensity
        let bloom_intensity = 3.0f32;
        let intensity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Intensity Buffer"),
            contents: bytemuck::cast_slice(&[bloom_intensity]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Additional buffers for adaptive bloom
        let adaptive_blur_scaling = 5.0f32;
        let adaptive_scaling_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Adaptive Scaling Buffer"),
                contents: bytemuck::cast_slice(&[adaptive_blur_scaling]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let max_blur_radius = 40.0f32;
        let max_radius_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Max Radius Buffer"),
            contents: bytemuck::cast_slice(&[max_blur_radius]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let intensity_curve = 5.0f32;
        let intensity_curve_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Intensity Curve Buffer"),
            contents: bytemuck::cast_slice(&[intensity_curve]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create shader modules
        let brightness_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Brightness Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/brightness.wgsl").into()),
        });

        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blur.wgsl").into()),
        });

        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/composite.wgsl").into()),
        });

        // Create bind group layouts
        let brightness_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Brightness Bind Group Layout"),
                entries: &[
                    // Texture binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu_types::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Threshold uniform binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Similar bind group layouts for blur and composite passes...
        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Blur Bind Group Layout"),
                entries: &[
                    // Texture binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu_types::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Direction uniform binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3, // This would be the next available binding
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let composite_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Composite Bind Group Layout"),
                entries: &[
                    // Scene texture binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Bloom texture binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu_types::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Intensity uniform binding
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4, // This would be the next available binding
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create bind groups
        let brightness_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Brightness Bind Group"),
            layout: &brightness_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        threshold_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        let blur_h_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Horizontal Blur Bind Group"),
            layout: &blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&brightness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        blur_h_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        adaptive_scaling_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(
                        max_radius_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        let blur_v_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Vertical Blur Bind Group"),
            layout: &blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&blur_h_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        blur_v_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        adaptive_scaling_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(
                        max_radius_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composite Bind Group"),
            layout: &composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blur_v_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        intensity_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer(
                        intensity_curve_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });

        // Create render pipeline layouts
        let brightness_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Brightness Pipeline Layout"),
                bind_group_layouts: &[&brightness_bind_group_layout],
                push_constant_ranges: &[],
            });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blur Pipeline Layout"),
            bind_group_layouts: &[&blur_bind_group_layout],
            push_constant_ranges: &[],
        });

        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Composite Pipeline Layout"),
                bind_group_layouts: &[&composite_bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipelines
        let brightness_pipeline = create_render_pipeline(
            device,
            &brightness_pipeline_layout,
            &brightness_shader,
            "Brightness Pipeline",
            wgpu::TextureFormat::Rgba16Float,
        );

        let blur_pipeline = create_render_pipeline(
            device,
            &blur_pipeline_layout,
            &blur_shader,
            "Blur Pipeline",
            wgpu::TextureFormat::Rgba16Float,
        );

        let composite_pipeline = create_render_pipeline(
            device,
            &composite_pipeline_layout,
            &composite_shader,
            "Composite Pipeline",
            wgpu::TextureFormat::Rgba16Float,
        );

        // Return the fully initialized PostProcessing struct
        Self {
            scene_texture,
            brightness_texture,
            blur_h_texture,
            blur_v_texture,
            composite_texture,
            scene_view,
            brightness_view,
            blur_h_view,
            blur_v_view,
            composite_view,
            sampler,
            brightness_pipeline,
            blur_pipeline,
            composite_pipeline,
            threshold_buffer,
            blur_h_buffer,
            blur_v_buffer,
            intensity_buffer,

            adaptive_scaling_buffer,
            max_radius_buffer,
            intensity_curve_buffer,

            brightness_threshold,
            bloom_intensity,
            adaptive_blur_scaling,
            max_blur_radius,
            intensity_curve,

            brightness_bind_group,
            blur_h_bind_group,
            blur_v_bind_group,
            composite_bind_group,
        }
    }

    pub fn process(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_view: &wgpu::TextureView,
        draw_renderer: &mut nannou::draw::Renderer,
        draw: &nannou::Draw,
    ) {
        // First, render the scene to the scene texture
        let ce_desc = wgpu::CommandEncoderDescriptor {
            label: Some("Scene renderer"),
        };
        let mut encoder = device.create_command_encoder(&ce_desc);

        draw_renderer.encode_render_pass(
            device,
            &mut encoder,
            draw,
            1.0,
            self.scene_texture.size(),
            &self.scene_view,
            None,
        );

        queue.submit(Some(encoder.finish()));

        // Now execute the post-processing passes

        // 1. Brightness extraction pass
        {
            let ce_desc = wgpu::CommandEncoderDescriptor {
                label: Some("Brightness extraction"),
            };
            let mut encoder = device.create_command_encoder(&ce_desc);

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Brightness pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.brightness_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.brightness_pipeline);
            pass.set_bind_group(0, &self.brightness_bind_group, &[]);
            pass.draw(0..3, 0..1); // Draw a fullscreen triangle

            drop(pass);
            queue.submit(Some(encoder.finish()));
        }

        // 2. Horizontal blur pass
        {
            let ce_desc = wgpu::CommandEncoderDescriptor {
                label: Some("Horizontal blur"),
            };
            let mut encoder = device.create_command_encoder(&ce_desc);

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Horizontal blur pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur_h_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &self.blur_h_bind_group, &[]);
            pass.draw(0..3, 0..1); // Draw a fullscreen triangle

            drop(pass);
            queue.submit(Some(encoder.finish()));
        }

        // 3. Vertical blur pass
        {
            let ce_desc = wgpu::CommandEncoderDescriptor {
                label: Some("Vertical blur"),
            };
            let mut encoder = device.create_command_encoder(&ce_desc);

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Vertical blur pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blur_v_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, &self.blur_v_bind_group, &[]);
            pass.draw(0..3, 0..1); // Draw a fullscreen triangle

            drop(pass);
            queue.submit(Some(encoder.finish()));
        }

        // 4. Final composite pass to the output texture
        {
            let ce_desc = wgpu::CommandEncoderDescriptor {
                label: Some("Final composite"),
            };
            let mut encoder = device.create_command_encoder(&ce_desc);

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: texture_view, // Render directly to the output
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &self.composite_bind_group, &[]);
            pass.draw(0..3, 0..1); // Draw a fullscreen triangle

            drop(pass);
            queue.submit(Some(encoder.finish()));
        }

        // Make sure all commands are completed
        device.poll(wgpu::Maintain::Wait);
    }

    /******************* Helper methods for updating parameters ****************** */

    pub fn set_brightness_threshold(&mut self, queue: &wgpu::Queue, threshold: f32) {
        self.brightness_threshold = threshold;
        queue.write_buffer(
            &self.threshold_buffer,
            0,
            bytemuck::cast_slice(&[threshold]),
        );
    }

    pub fn set_bloom_intensity(&mut self, queue: &wgpu::Queue, intensity: f32) {
        self.bloom_intensity = intensity;
        queue.write_buffer(
            &self.intensity_buffer,
            0,
            bytemuck::cast_slice(&[intensity]),
        );
    }

    pub fn set_adaptive_blur_scaling(&mut self, queue: &wgpu::Queue, scaling: f32) {
        self.adaptive_blur_scaling = scaling;
        queue.write_buffer(
            &self.adaptive_scaling_buffer,
            0,
            bytemuck::cast_slice(&[scaling]),
        );
    }

    pub fn set_max_blur_radius(&mut self, queue: &wgpu::Queue, radius: f32) {
        self.max_blur_radius = radius;
        queue.write_buffer(&self.max_radius_buffer, 0, bytemuck::cast_slice(&[radius]));
    }

    pub fn set_intensity_curve(&mut self, queue: &wgpu::Queue, curve: f32) {
        self.intensity_curve = curve;
        queue.write_buffer(
            &self.intensity_curve_buffer,
            0,
            bytemuck::cast_slice(&[curve]),
        );
    }
}

// Helper function to create render texture
fn create_render_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    samples: u32,
) -> wgpu::Texture {
    wgpu::TextureBuilder::new()
        .size([width, height])
        .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        .sample_count(samples)
        .format(wgpu::TextureFormat::Rgba16Float)
        .build(device)
}

// Helper function to create render pipeline
fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    label: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}
