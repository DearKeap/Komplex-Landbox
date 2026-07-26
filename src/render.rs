use crate::world::chunk::{block_color, is_solid, ChunkSection, CHUNK_SIZE};
use glam::Mat4;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

struct FaceDef {
    normal: [i32; 3],
    // 4 sudut per face, urutan CCW dilihat dari arah normal (buat backface culling)
    corners: [[f32; 3]; 4],
}

const FACES: [FaceDef; 6] = [
    FaceDef { normal: [1, 0, 0], corners: [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]] }, // +X
    FaceDef { normal: [-1, 0, 0], corners: [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]] }, // -X
    FaceDef { normal: [0, 1, 0], corners: [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]] }, // +Y
    FaceDef { normal: [0, -1, 0], corners: [[0., 0., 1.], [0., 0., 0.], [1., 0., 0.], [1., 0., 1.]] }, // -Y
    FaceDef { normal: [0, 0, 1], corners: [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]] }, // +Z
    FaceDef { normal: [0, 0, -1], corners: [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]] }, // -Z
];

/// Mesher NAIVE — satu quad per face yang keliatan (neighbor-nya air/di luar batas).
/// Belum greedy meshing (gabung face sejenis jadi satu quad besar) — itu optimasi
/// lanjutan, bukan kebutuhan buat "jalan dulu".
pub fn mesh_chunk_section(section: &ChunkSection) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let block = section.get_block(x, y, z);
                if !is_solid(block) {
                    continue;
                }
                let color = block_color(block);

                for face in &FACES {
                    let nx = x as i32 + face.normal[0];
                    let ny = y as i32 + face.normal[1];
                    let nz = z as i32 + face.normal[2];

                    let neighbor_solid = if nx < 0
                        || ny < 0
                        || nz < 0
                        || nx >= CHUNK_SIZE as i32
                        || ny >= CHUNK_SIZE as i32
                        || nz >= CHUNK_SIZE as i32
                    {
                        false // di luar section dianggap kosong — batas chunk keliatan dari luar
                    } else {
                        is_solid(section.get_block(nx as usize, ny as usize, nz as usize))
                    };

                    if neighbor_solid {
                        continue; // face ketutup, gak usah digambar
                    }

                    let base_index = vertices.len() as u32;
                    for corner in &face.corners {
                        vertices.push(Vertex {
                            position: [x as f32 + corner[0], y as f32 + corner[1], z as f32 + corner[2]],
                            color,
                        });
                    }
                    indices.extend_from_slice(&[
                        base_index, base_index + 1, base_index + 2,
                        base_index, base_index + 2, base_index + 3,
                    ]);
                }
            }
        }
    }

    (vertices, indices)
}

/// Semua state pipeline render: shader, layout, depth buffer, uniform kamera.
pub struct RenderPipelineBundle {
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub depth_texture: wgpu::TextureView,
}

impl RenderPipelineBundle {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kombox-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniform"),
            size: std::mem::size_of::<[f32; 16]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kombox-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let depth_texture = create_depth_texture(device, config);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kombox-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self { pipeline, uniform_buffer, bind_group, depth_texture }
    }

    pub fn resize_depth(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        self.depth_texture = create_depth_texture(device, config);
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, view_proj: Mat4) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&view_proj.to_cols_array()));
    }
}

fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: config.width.max(1),
        height: config.height.max(1),
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth-texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
