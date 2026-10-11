use wgpu::{
    Adapter, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, BufferSize, ColorTargetState, ColorWrites, Device, DeviceDescriptor, ErrorFilter,
    ErrorScopeGuard, ExperimentalFeatures, Features, FragmentState, FrontFace, Limits, MemoryHints, MultisampleState,
    PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, TextureFormat, Trace, VertexBufferLayout, VertexState,
};

use crate::error::AppError;
use crate::render::CameraUniform;
use crate::render::{cell_pipeline, shape_pipeline};

/// Per instance; one triangle strip.
pub const QUAD_VERTEX_COUNT: u32 = 4;

/// Must match the camera's `@group` in scene_common.wgsl.
pub const CAMERA_BIND_GROUP_INDEX: u32 = 0;

/// Must match the camera's `@binding` in scene_common.wgsl.
pub const CAMERA_UNIFORM_BINDING: u32 = 0;

/// Must match the position of `instance_layout` in the pipeline's vertex `buffers`.
pub const INSTANCE_VERTEX_BUFFER_SLOT: u32 = 0;

pub struct ScenePipelines {
    pub camera_bind_group_layout: BindGroupLayout,
    pub cell: RenderPipeline,
    pub shape: RenderPipeline,
}

impl ScenePipelines {
    pub async fn create(device: &Device, surface_format: TextureFormat) -> Result<ScenePipelines, AppError> {
        let error_scopes: [ErrorScopeGuard; 3] = [
            device.push_error_scope(ErrorFilter::OutOfMemory),
            device.push_error_scope(ErrorFilter::Internal),
            device.push_error_scope(ErrorFilter::Validation),
        ];

        let camera_bind_group_layout: BindGroupLayout = create_camera_bind_group_layout(device);
        let pipeline_layout: PipelineLayout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("bacter-scene-pipeline-layout"),
            bind_group_layouts: &[Some(&camera_bind_group_layout)],
            immediate_size: 0,
        });
        let cell: RenderPipeline = cell_pipeline::create_cell_pipeline(device, &pipeline_layout, surface_format);
        let shape: RenderPipeline = shape_pipeline::create_shape_pipeline(device, &pipeline_layout, surface_format);

        let first_error: Option<wgpu::Error> = drain_error_scopes(error_scopes).await;

        if let Some(error) = first_error {
            return Err(AppError::new(&format!("building the scene pipelines failed: {error}")));
        }

        Ok(ScenePipelines {
            camera_bind_group_layout,
            cell,
            shape,
        })
    }
}

/// One pipeline's shader and per-instance vertex buffer.
pub struct InstancedQuadPipelineSource<'source> {
    pub label: &'source str,
    /// `scene_common.wgsl` followed by the pipeline's own WGSL.
    pub shader_source: &'static str,
    pub vertex_entry_point: &'source str,
    pub fragment_entry_point: &'source str,
    pub instance_layout: VertexBufferLayout<'source>,
}

/// WebGL2's limits on every backend, so that what runs here runs on WebGL2, except the adapter's texture dimensions,
/// which a full-window surface at a high device pixel ratio needs.
pub fn get_device_descriptor(adapter: &Adapter) -> DeviceDescriptor<'static> {
    DeviceDescriptor {
        label: Some("bacter-renderer-device"),
        required_features: Features::empty(),
        required_limits: Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
        experimental_features: ExperimentalFeatures::default(),
        memory_hints: MemoryHints::Performance,
        trace: Trace::Off,
    }
}

/// The shaders return premultiplied colours.
pub fn create_instanced_quad_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    surface_format: TextureFormat,
    pipeline_source: InstancedQuadPipelineSource,
) -> RenderPipeline {
    let shader_module: ShaderModule = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(pipeline_source.label),
        source: ShaderSource::Wgsl(pipeline_source.shader_source.into()),
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(pipeline_source.label),
        layout: Some(pipeline_layout),
        vertex: VertexState {
            module: &shader_module,
            entry_point: Some(pipeline_source.vertex_entry_point),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[Some(pipeline_source.instance_layout)],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(FragmentState {
            module: &shader_module,
            entry_point: Some(pipeline_source.fragment_entry_point),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_camera_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("bacter-camera-bind-group-layout"),
        entries: &[BindGroupLayoutEntry {
            binding: CAMERA_UNIFORM_BINDING,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(std::mem::size_of::<CameraUniform>() as u64),
            },
            count: None,
        }],
    })
}

/// The first error any scope captured.
async fn drain_error_scopes(error_scopes: [ErrorScopeGuard; 3]) -> Option<wgpu::Error> {
    let mut first_error: Option<wgpu::Error> = None;

    // wgpu requires every pushed scope to be popped, innermost first.
    for error_scope in error_scopes.into_iter().rev() {
        let error: Option<wgpu::Error> = error_scope.pop().await;
        first_error = first_error.or(error);
    }

    first_error
}

#[cfg(test)]
mod tests {
    use std::pin::{Pin, pin};
    use std::task::{Context, Poll, Waker};

    use wgpu::{Adapter, Instance, PowerPreference, Queue, RequestAdapterOptions};

    use super::*;

    /// wgpu's futures on these targets are ready when first polled.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut pinned_future: Pin<&mut F> = pin!(future);
        let mut context: Context = Context::from_waker(Waker::noop());

        loop {
            let polled: Poll<F::Output> = pinned_future.as_mut().poll(&mut context);

            if let Poll::Ready(output) = polled {
                return output;
            }

            std::thread::yield_now();
        }
    }

    fn create_headless_device() -> Device {
        let instance: Instance = Instance::default();
        let adapter: Adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .unwrap();
        let (device, _queue): (Device, Queue) =
            block_on(adapter.request_device(&get_device_descriptor(&adapter))).unwrap();

        device
    }

    #[test]
    #[ignore = "needs a GPU adapter; run by ./scripts/test/test-shaders.sh"]
    fn create_builds_every_pipeline_on_a_headless_device() {
        let device: Device = create_headless_device();

        for surface_format in [TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm] {
            let created: Result<ScenePipelines, AppError> = block_on(ScenePipelines::create(&device, surface_format));

            created.unwrap();
        }
    }
}
