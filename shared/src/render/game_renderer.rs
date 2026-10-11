use std::marker::PhantomData;

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, AdapterInfo, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferUsages, Color,
    CommandEncoder, CommandEncoderDescriptor, CurrentSurfaceTexture, Device, Instance, InstanceDescriptor, LoadOp,
    Operations, PowerPreference, Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RequestAdapterOptions, StoreOp, SurfaceTexture, TextureView, TextureViewDescriptor,
};

#[cfg(target_arch = "wasm32")] // other targets have no canvas element
use web_sys::HtmlCanvasElement;

use crate::error::AppError;
use crate::render::render_color;
use crate::render::scene_pipelines;
use crate::render::{
    CameraUniform, CellInstance, ConfiguredSurface, FrameLayer, InstanceBuffer, PhysicalSize, RenderColor, RenderScene,
    ScenePipelines, ShapeInstance, TickLayer, WgpuSurface,
};

#[cfg(not(target_arch = "wasm32"))] // wasm32 renders into a canvas, not a native view
use crate::render::WindowHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererBackendKind {
    /// WebGPU where the browser has it, else WebGL2; the native GPU API elsewhere.
    Default,
    ForceGl,
}

/// `!Send`: wgpu resources on wasm32 belong to the thread which created them.
pub struct GameRenderer {
    adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: ConfiguredSurface,
    pipelines: ScenePipelines,
    camera_buffer: Buffer,
    camera_bind_group: BindGroup,
    tick_layer_buffers: LayerBuffers,
    frame_layer_buffers: LayerBuffers,
    not_send: PhantomData<*const ()>,
}

impl GameRenderer {
    #[cfg(target_arch = "wasm32")] // other targets have no canvas element
    pub async fn from_canvas(
        canvas: HtmlCanvasElement,
        backend: RendererBackendKind,
        size: PhysicalSize,
    ) -> Result<GameRenderer, AppError> {
        let instance: Instance = create_instance(backend);
        let surface: WgpuSurface = WgpuSurface::from_canvas(&instance, canvas)?;

        GameRenderer::from_surface(&instance, surface, size).await
    }

    /// # Safety
    ///
    /// The view must be valid and outlive the renderer.
    #[cfg(not(target_arch = "wasm32"))] // wasm32 renders into a canvas, not a native view
    pub async unsafe fn from_window_handle(
        window_handle: WindowHandle,
        backend: RendererBackendKind,
        size: PhysicalSize,
    ) -> Result<GameRenderer, AppError> {
        let instance: Instance = create_instance(backend);
        // SAFETY: the caller guarantees the view is valid and outlives the renderer.
        let surface: WgpuSurface = unsafe { WgpuSurface::from_window_handle(&instance, window_handle) }?;

        GameRenderer::from_surface(&instance, surface, size).await
    }

    async fn from_surface(
        instance: &Instance,
        surface: WgpuSurface,
        size: PhysicalSize,
    ) -> Result<GameRenderer, AppError> {
        // WebGL2 needs the adapter requested compatible with the surface.
        let requested_adapter: Result<Adapter, wgpu::RequestAdapterError> = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(surface.inner()),
                apply_limit_buckets: false,
            })
            .await;
        let adapter: Adapter =
            requested_adapter.map_err(|error| AppError::new(&format!("requesting a GPU adapter failed: {error}")))?;

        let requested_device: Result<(Device, Queue), wgpu::RequestDeviceError> =
            adapter.request_device(&scene_pipelines::get_device_descriptor(&adapter)).await;
        let (device, queue): (Device, Queue) =
            requested_device.map_err(|error| AppError::new(&format!("requesting a GPU device failed: {error}")))?;

        let configured_surface: ConfiguredSurface = surface.configure(&adapter, &device, size)?;
        let pipelines: ScenePipelines = ScenePipelines::create(&device, configured_surface.format()).await?;
        let camera_buffer: Buffer = create_camera_buffer(&device);
        let camera_bind_group: BindGroup = create_camera_bind_group(&device, &pipelines, &camera_buffer);
        let tick_layer_buffers: LayerBuffers = LayerBuffers::new(&device, "bacter-tick-shapes", "bacter-tick-cells");
        let frame_layer_buffers: LayerBuffers = LayerBuffers::new(&device, "bacter-frame-shapes", "bacter-frame-cells");

        Ok(GameRenderer {
            adapter,
            device,
            queue,
            surface: configured_surface,
            pipelines,
            camera_buffer,
            camera_bind_group,
            tick_layer_buffers,
            frame_layer_buffers,
            not_send: PhantomData,
        })
    }

    pub fn adapter_info(&self) -> AdapterInfo {
        self.adapter.get_info()
    }

    pub fn resize(&mut self, size: PhysicalSize) {
        self.surface.resize(&self.device, size);
    }

    /// Kept for every `draw_frame` until the next upload.
    pub fn upload_tick_layer(&mut self, tick_layer: &TickLayer) {
        self.tick_layer_buffers.write(
            &self.device,
            &self.queue,
            &tick_layer.background_shapes,
            &tick_layer.organism_cells,
        );
    }

    /// Draws the last uploaded tick layer under the scene's frame layer.
    pub fn draw_frame(&mut self, render_scene: &RenderScene) -> Result<(), AppError> {
        let acquired_surface_texture: Option<SurfaceTexture> = self.acquire_surface_texture()?;
        let Some(surface_texture) = acquired_surface_texture else {
            return Ok(());
        };

        self.write_frame_layer(&render_scene.frame_layer);
        self.write_camera_uniform(render_scene.camera.to_uniform());

        let view: TextureView = surface_texture.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder: CommandEncoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("bacter-scene-encoder"),
        });

        self.record_scene_pass(&mut encoder, &view);

        self.queue.submit([encoder.finish()]);
        self.queue.present(surface_texture);

        Ok(())
    }

    /// `None` for a frame to skip.
    fn acquire_surface_texture(&self) -> Result<Option<SurfaceTexture>, AppError> {
        let acquired: CurrentSurfaceTexture = self.surface.inner().get_current_texture();

        match acquired {
            CurrentSurfaceTexture::Success(surface_texture) => Ok(Some(surface_texture)),
            CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                // Dropping discards the texture; a surface with an acquired texture cannot be configured.
                drop(surface_texture);
                self.surface.reconfigure(&self.device);

                Ok(None)
            }
            CurrentSurfaceTexture::Outdated => {
                self.surface.reconfigure(&self.device);

                Ok(None)
            }
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => Ok(None),
            CurrentSurfaceTexture::Lost => Err(AppError::new(
                "the surface was lost; the renderer must be created again",
            )),
            CurrentSurfaceTexture::Validation => Err(AppError::new("acquiring the surface texture failed validation")),
        }
    }

    fn write_frame_layer(&mut self, frame_layer: &FrameLayer) {
        self.frame_layer_buffers.write(
            &self.device,
            &self.queue,
            &frame_layer.crosshair_shapes,
            &frame_layer.projectile_cells,
        );
    }

    fn write_camera_uniform(&self, camera_uniform: CameraUniform) {
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera_uniform]));
    }

    fn record_scene_pass(&self, encoder: &mut CommandEncoder, view: &TextureView) {
        let mut render_pass: RenderPass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("bacter-scene-pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(get_clear_color(render_color::BACKDROP_COLOR)),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_bind_group(scene_pipelines::CAMERA_BIND_GROUP_INDEX, &self.camera_bind_group, &[]);

        draw_instances(&mut render_pass, &self.pipelines.shape, &self.tick_layer_buffers.shapes);
        draw_instances(&mut render_pass, &self.pipelines.cell, &self.tick_layer_buffers.cells);
        draw_instances(&mut render_pass, &self.pipelines.cell, &self.frame_layer_buffers.cells);
        draw_instances(
            &mut render_pass,
            &self.pipelines.shape,
            &self.frame_layer_buffers.shapes,
        );
    }
}

/// One layer's instances on the GPU.
struct LayerBuffers {
    shapes: InstanceBuffer<ShapeInstance>,
    cells: InstanceBuffer<CellInstance>,
}

impl LayerBuffers {
    fn new(device: &Device, shapes_label: &'static str, cells_label: &'static str) -> LayerBuffers {
        LayerBuffers {
            shapes: InstanceBuffer::new(device, shapes_label),
            cells: InstanceBuffer::new(device, cells_label),
        }
    }

    fn write(&mut self, device: &Device, queue: &Queue, shapes: &[ShapeInstance], cells: &[CellInstance]) {
        self.shapes.write(device, queue, shapes);
        self.cells.write(device, queue, cells);
    }
}

/// Channels unconverted, for a surface format without sRGB encoding.
pub fn get_clear_color(color: RenderColor) -> Color {
    Color {
        r: f64::from(color.red) / f64::from(u8::MAX),
        g: f64::from(color.green) / f64::from(u8::MAX),
        b: f64::from(color.blue) / f64::from(u8::MAX),
        a: f64::from(color.alpha) / f64::from(u8::MAX),
    }
}

fn create_instance(backend: RendererBackendKind) -> Instance {
    match backend {
        RendererBackendKind::Default => Instance::default(),
        RendererBackendKind::ForceGl => {
            let mut descriptor: InstanceDescriptor = InstanceDescriptor::new_without_display_handle();
            descriptor.backends = Backends::GL;

            Instance::new(descriptor)
        }
    }
}

fn create_camera_buffer(device: &Device) -> Buffer {
    device.create_buffer_init(&BufferInitDescriptor {
        label: Some("bacter-camera-uniform"),
        contents: bytemuck::cast_slice(&[CameraUniform {
            world_to_clip: [[0.0; 4]; 4],
            physical_pixels_per_world_pixel: 1.0,
            padding: [0.0; 3],
        }]),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    })
}

fn create_camera_bind_group(device: &Device, pipelines: &ScenePipelines, camera_buffer: &Buffer) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("bacter-camera-bind-group"),
        layout: &pipelines.camera_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: scene_pipelines::CAMERA_UNIFORM_BINDING,
            resource: camera_buffer.as_entire_binding(),
        }],
    })
}

fn draw_instances<T: bytemuck::Pod>(
    render_pass: &mut RenderPass,
    pipeline: &RenderPipeline,
    instance_buffer: &InstanceBuffer<T>,
) {
    let Some(written_slice) = instance_buffer.written_slice() else {
        return;
    };

    render_pass.set_pipeline(pipeline);
    render_pass.set_vertex_buffer(scene_pipelines::INSTANCE_VERTEX_BUFFER_SLOT, written_slice);
    render_pass.draw(0..scene_pipelines::QUAD_VERTEX_COUNT, 0..instance_buffer.count());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_clear_color_scales_every_channel_to_one() {
        assert_eq!(
            get_clear_color(RenderColor::opaque(0, 51, 255)),
            Color {
                r: 0.0,
                g: 0.2,
                b: 1.0,
                a: 1.0,
            },
        );
    }
}
