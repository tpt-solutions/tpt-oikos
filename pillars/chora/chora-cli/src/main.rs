use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::EventLoop,
    window::Window,
};

use chora_render::{GpuContext, RenderGraph, SceneGraph, TrianglePipeline, Vertex};
use chora_data::{MandateState, Balance};

struct AppState {
    gpu: Option<GpuContext>,
    scene: SceneGraph,
    render_graph: RenderGraph,
    triangle_pipeline: Option<TrianglePipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    num_indices: u32,
}

impl AppState {
    fn new() -> Self {
        Self {
            gpu: None,
            scene: SceneGraph::new(),
            render_graph: RenderGraph::new(),
            triangle_pipeline: None,
            vertex_buffer: None,
            index_buffer: None,
            num_indices: 0,
        }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let gpu = pollster::block_on(GpuContext::new(window));
        let pipeline = TrianglePipeline::new(&gpu.device, gpu.surface_config.format);

        let vertices = vec![
            Vertex { position: [0.0, 0.5], color: [1.0, 0.0, 0.0] },
            Vertex { position: [-0.5, -0.5], color: [0.0, 1.0, 0.0] },
            Vertex { position: [0.5, -0.5], color: [0.0, 0.0, 1.0] },
        ];

        let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("triangle_vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("triangle_index_buffer"),
            contents: bytemuck::cast_slice(&[0u16, 1, 2]),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.gpu = Some(gpu);
        self.triangle_pipeline = Some(pipeline);
        self.vertex_buffer = Some(vertex_buffer);
        self.index_buffer = Some(index_buffer);
        self.num_indices = 3;
    }
}

impl ApplicationHandler for AppState {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.gpu.is_none() {
            let attrs = Window::default_attributes()
                .with_title("tpt-chora — Human Observation Runtime")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.init_gpu(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(new_size.width, new_size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    let output = match gpu.surface.get_current_texture() {
                        Ok(t) => t,
                        Err(wgpu::SurfaceError::Lost) => {
                            gpu.surface.configure(&gpu.device, &gpu.surface_config);
                            return;
                        }
                        Err(e) => {
                            eprintln!("Surface error: {e}");
                            return;
                        }
                    };

                    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = gpu.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("chora-frame-encoder"),
                        },
                    );

                    {
                        if let (Some(pipeline), Some(vb), Some(ib)) = (
                            &self.triangle_pipeline,
                            &self.vertex_buffer,
                            &self.index_buffer,
                        ) {
                            let mut render_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("triangle_pass"),
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                                r: 0.02,
                                                g: 0.02,
                                                b: 0.05,
                                                a: 1.0,
                                            }),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    ..Default::default()
                                });

                            render_pass.set_pipeline(&pipeline.render_pipeline);
                            render_pass.set_vertex_buffer(0, vb.slice(..));
                            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
                        }
                    }

                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut state = AppState::new();

    let _mandate = MandateState::new("mandate-001", "treasury", "agent-alpha", 10000.0);
    let _balance = Balance::new("agent-alpha", 5000.0, "ETH");

    eprintln!("tpt-chora v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("Human Observation Runtime — launching renderer");

    event_loop.run_app(&mut state).unwrap();
}
