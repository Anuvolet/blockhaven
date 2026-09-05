mod render;

use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Blockhaven")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .expect("window"),
    );
    let mut gpu = render::gpu::Gpu::new(window.clone(), true);
    println!("GPU: {} ({})", gpu.adapter_name, gpu.backend);

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => gpu.resize(size.width, size.height),
                WindowEvent::RedrawRequested => {
                    let frame = match gpu.surface.get_current_texture() {
                        Ok(f) => f,
                        Err(_) => {
                            gpu.resize(gpu.config.width, gpu.config.height);
                            return;
                        }
                    };
                    let view = frame.texture.create_view(&Default::default());
                    let mut enc = gpu.device.create_command_encoder(&Default::default());
                    {
                        let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("clear"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.45,
                                        g: 0.68,
                                        b: 0.95,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    }
                    gpu.queue.submit(Some(enc.finish()));
                    frame.present();
                }
                _ => {}
            },
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        })
        .expect("event loop run");
}
