mod camera;
mod hook;
mod render;
mod tick;
mod world;

use std::collections::HashSet;
use std::sync::Arc;

use camera::Camera;
use glam::Vec3;
use render::{mesh_chunk_section, RenderPipelineBundle};
use tick::{TickClock, TickRate};
use wgpu::util::DeviceExt;
use winit::{
    event::{DeviceEvent, ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowBuilder},
};
use world::{chunk::CHUNK_SIZE, World};

const DEFAULT_SEED: u32 = 1337;
const WORLD_CHUNKS: i32 = 4; // grid 4x4 chunk = 64x16x64 block

struct ChunkMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

impl GpuState {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("gagal bikin surface — cek versi wgpu vs kode ini (lihat README)");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("gak ketemu GPU adapter yang cocok");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kombox-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("gagal request device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox]
            .into_iter()
            .find(|m| surface_caps.present_modes.contains(m))
            .unwrap_or(surface_caps.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!("Present mode dipakai: {:?}", present_mode);

        Self { surface, device, queue, config, size }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&self, pipeline: &RenderPipelineBundle, meshes: &[ChunkMesh]) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("kombox-encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.53, g: 0.75, b: 0.92, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &pipeline.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&pipeline.pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);

            for mesh in meshes {
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn set_cursor_locked(window: &Window, locked: bool) {
    if locked {
        window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .ok();
        window.set_cursor_visible(false);
    } else {
        window.set_cursor_grab(CursorGrabMode::None).ok();
        window.set_cursor_visible(true);
    }
}

fn generate_chunk_meshes(gpu: &GpuState, world: &World) -> Vec<ChunkMesh> {
    let mut meshes = Vec::new();
    for (&(cx, cz), section) in world.iter_chunks() {
        let (mut vertices, indices) = mesh_chunk_section(section);
        if indices.is_empty() {
            continue;
        }
        let offset_x = (cx * CHUNK_SIZE as i32) as f32;
        let offset_z = (cz * CHUNK_SIZE as i32) as f32;
        for v in &mut vertices {
            v.position[0] += offset_x;
            v.position[2] += offset_z;
        }

        let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk-vbuf"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk-ibuf"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        meshes.push(ChunkMesh { vertex_buffer, index_buffer, num_indices: indices.len() as u32 });
    }
    meshes
}

fn main() {
    env_logger::init();

    println!("===========================================================");
    println!("  KOMBOX VOXEL ENGINE - Smooth Physics & 3-Mode TPS UI     ");
    println!("===========================================================");
    println!("  CONTROLS:");
    println!("    W, A, S, D  : Smooth Movement (Walk / Air Acceleration)");
    println!("    Space       : Smooth Jump");
    println!("    Mouse Look  : Rotate Camera View");
    println!("    Esc         : Unlock / Lock Cursor");
    println!("-----------------------------------------------------------");
    println!("  UI & SEED CONTROLS:");
    println!("    [1]         : Switch to Potato Mode (20 TPS)");
    println!("    [2]         : Switch to Rice Mode (50 TPS)");
    println!("    [3]         : Switch to Beef Mode (100 TPS)");
    println!("    [R]         : Randomize / Regenerate Seed World");
    println!("===========================================================");

    let event_loop = EventLoop::new().expect("gagal bikin event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Kombox — Seed: 1337 | Potato (20 TPS)")
            .build(&event_loop)
            .expect("gagal bikin window"),
    );

    let mut gpu = pollster::block_on(GpuState::new(window.clone()));
    let mut pipeline = RenderPipelineBundle::new(&gpu.device, &gpu.config);

    let mut current_seed = DEFAULT_SEED;
    let mut world = World::generate(WORLD_CHUNKS, WORLD_CHUNKS, current_seed);
    let mut meshes = generate_chunk_meshes(&gpu, &world);

    log::info!("World digenerate: {} chunk mesh non-kosong dari {}x{} (Seed: {})", meshes.len(), WORLD_CHUNKS, WORLD_CHUNKS, current_seed);

    let mut hooks = hook::HookRegistry::new();
    hooks.register(":entity.death", Box::new(|ctx| log::info!("hook fired: {:?}", ctx)));
    hooks.fire(":entity.death", &hook::HookContext::new().with("entity_id", "1"));

    // Default TPS Mode: Potato (20 TPS)
    let mut clock = TickClock::new(TickRate::Potato);
    let world_center = (WORLD_CHUNKS * CHUNK_SIZE as i32) as f32 * 0.5;
    let mut camera = Camera::new(Vec3::new(world_center, 25.0, world_center - 10.0));
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();

    let mut cursor_locked = true;
    set_cursor_locked(&window, cursor_locked);

    let mut fps_frame_count: u32 = 0;
    let mut current_fps: f32 = 0.0;
    let mut fps_last_report = std::time::Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(new_size) => {
                    gpu.resize(new_size);
                    pipeline.resize_depth(&gpu.device, &gpu.config);
                }
                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    if let PhysicalKey::Code(code) = key_event.physical_key {
                        if key_event.state == ElementState::Pressed {
                            match code {
                                KeyCode::Escape => {
                                    cursor_locked = !cursor_locked;
                                    set_cursor_locked(&window, cursor_locked);
                                }
                                KeyCode::Digit1 => {
                                    clock.set_mode(TickRate::Potato);
                                    println!("[UI SELECTION]: Switched TPS Mode -> {}", clock.mode.label());
                                }
                                KeyCode::Digit2 => {
                                    clock.set_mode(TickRate::Rice);
                                    println!("[UI SELECTION]: Switched TPS Mode -> {}", clock.mode.label());
                                }
                                KeyCode::Digit3 => {
                                    clock.set_mode(TickRate::Beef);
                                    println!("[UI SELECTION]: Switched TPS Mode -> {}", clock.mode.label());
                                }
                                KeyCode::KeyR => {
                                    current_seed = current_seed.wrapping_add(1013);
                                    world = World::generate(WORLD_CHUNKS, WORLD_CHUNKS, current_seed);
                                    meshes = generate_chunk_meshes(&gpu, &world);
                                    println!("[UI SELECTION]: Regenerated World Seed -> {}", current_seed);
                                }
                                _ => {}
                            }
                        }

                        match key_event.state {
                            ElementState::Pressed => { pressed_keys.insert(code); }
                            ElementState::Released => { pressed_keys.remove(&code); }
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let ticks = clock.ticks_to_run();
                    for _ in 0..ticks {
                        camera.update(&pressed_keys, &world, clock.dt());
                    }

                    let aspect = gpu.config.width as f32 / gpu.config.height.max(1) as f32;
                    pipeline.update_camera(&gpu.queue, camera.view_proj(aspect));

                    match gpu.render(&pipeline, &meshes) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => gpu.resize(gpu.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => log::warn!("render error: {:?}", e),
                    }
                    window.request_redraw();

                    fps_frame_count += 1;
                    let elapsed = fps_last_report.elapsed();
                    if elapsed.as_secs_f32() >= 1.0 {
                        current_fps = fps_frame_count as f32 / elapsed.as_secs_f32();
                        let title = format!(
                            "Kombox | Seed: {} | TPS Mode: {} | FPS: {:.0} | Pos: ({:.1}, {:.1}, {:.1})",
                            current_seed,
                            clock.mode.label(),
                            current_fps,
                            camera.position.x,
                            camera.position.y,
                            camera.position.z
                        );
                        window.set_title(&title);
                        fps_frame_count = 0;
                        fps_last_report = std::time::Instant::now();
                    }
                }
                _ => {}
            },
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                if cursor_locked {
                    camera.apply_mouse_delta(delta.0 as f32, delta.1 as f32);
                }
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop error");
}
