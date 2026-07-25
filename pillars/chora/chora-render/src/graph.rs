pub struct RenderGraph {
    passes: Vec<RenderPass>,
}

pub struct RenderPass {
    pub name: String,
    pub pipeline_id: usize,
    pub vertex_count: u32,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add_pass(&mut self, pass: RenderPass) {
        self.passes.push(pass);
    }

    pub fn execute(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        pipelines: &[wgpu::RenderPipeline],
        bind_groups: &[&wgpu::BindGroup],
    ) {
        for pass in &self.passes {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&pass.name),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            if pass.pipeline_id < pipelines.len() {
                render_pass.set_pipeline(&pipelines[pass.pipeline_id]);
                for (i, bg) in bind_groups.iter().enumerate() {
                    render_pass.set_bind_group(i as u32, *bg, &[]);
                }
                render_pass.draw(0..pass.vertex_count, 0..1);
            }
        }
    }

    pub fn clear(&mut self) {
        self.passes.clear();
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}
