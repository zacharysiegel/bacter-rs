use wgpu::{
    Adapter, CompositeAlphaMode, Device, PresentMode, Surface, SurfaceCapabilities, SurfaceColorSpace,
    SurfaceConfiguration, TextureFormat, TextureUsages,
};

use crate::error::AppError;
use crate::render::PhysicalSize;

#[cfg(not(target_arch = "wasm32"))] // wasm32 renders into a canvas, not a native view
pub use native::WindowHandle;

/// Not yet configured.
pub struct WgpuSurface {
    inner: Surface<'static>,
}

impl WgpuSurface {
    pub fn inner(&self) -> &Surface<'static> {
        &self.inner
    }

    pub fn configure(
        self,
        adapter: &Adapter,
        device: &Device,
        size: PhysicalSize,
    ) -> Result<ConfiguredSurface, AppError> {
        let capabilities: SurfaceCapabilities = self.inner.get_capabilities(adapter);
        let format: TextureFormat = choose_surface_format(&capabilities.formats)
            .ok_or_else(|| AppError::new("the surface supports no format with this adapter"))?;
        let alpha_mode: CompositeAlphaMode =
            capabilities.alpha_modes.first().copied().unwrap_or(CompositeAlphaMode::Auto);
        let configured_size: PhysicalSize = size.at_least_one_pixel();
        let configuration: SurfaceConfiguration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: SurfaceColorSpace::Auto,
            width: configured_size.width_pixels,
            height: configured_size.height_pixels,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };

        self.inner.configure(device, &configuration);

        Ok(ConfiguredSurface {
            inner: self.inner,
            configuration,
        })
    }
}

pub struct ConfiguredSurface {
    inner: Surface<'static>,
    configuration: SurfaceConfiguration,
}

impl ConfiguredSurface {
    pub fn inner(&self) -> &Surface<'static> {
        &self.inner
    }

    pub fn format(&self) -> TextureFormat {
        self.configuration.format
    }

    pub fn resize(&mut self, device: &Device, size: PhysicalSize) {
        let configured_size: PhysicalSize = size.at_least_one_pixel();
        self.configuration.width = configured_size.width_pixels;
        self.configuration.height = configured_size.height_pixels;

        self.inner.configure(device, &self.configuration);
    }

    pub fn reconfigure(&self, device: &Device) {
        self.inner.configure(device, &self.configuration);
    }
}

/// The first format without sRGB encoding, else the first format; the original's canvas blended sRGB values.
pub fn choose_surface_format(formats: &[TextureFormat]) -> Option<TextureFormat> {
    let unencoded_format: Option<TextureFormat> = formats.iter().copied().find(|format| !format.is_srgb());

    unencoded_format.or_else(|| formats.first().copied())
}

#[cfg(target_arch = "wasm32")] // other targets have no canvas element
mod wasm {
    use web_sys::HtmlCanvasElement;
    use wgpu::{CreateSurfaceError, Instance, Surface, SurfaceTarget};

    use super::WgpuSurface;
    use crate::error::AppError;

    impl WgpuSurface {
        pub fn from_canvas(instance: &Instance, canvas: HtmlCanvasElement) -> Result<WgpuSurface, AppError> {
            let created: Result<Surface<'static>, CreateSurfaceError> =
                instance.create_surface(SurfaceTarget::Canvas(canvas));
            let surface: Surface<'static> = created
                .map_err(|error| AppError::new(&format!("creating a surface from the canvas failed: {error}")))?;

            Ok(WgpuSurface { inner: surface })
        }
    }
}

#[cfg(not(target_arch = "wasm32"))] // wasm32 renders into a canvas, not a native view
mod native {
    use core::ffi::c_void;
    use core::ptr::NonNull;

    use raw_window_handle::{RawDisplayHandle, RawWindowHandle, UiKitDisplayHandle, UiKitWindowHandle};
    use wgpu::{CreateSurfaceError, Instance, Surface, SurfaceTargetUnsafe};

    use super::WgpuSurface;
    use crate::error::AppError;

    /// A view owned by the native shell, passed as its address.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum WindowHandle {
        UiKit { ui_view_address: u64 },
    }

    impl WgpuSurface {
        /// # Safety
        ///
        /// The view must be valid and outlive the surface.
        pub unsafe fn from_window_handle(
            instance: &Instance,
            window_handle: WindowHandle,
        ) -> Result<WgpuSurface, AppError> {
            let target: SurfaceTargetUnsafe = get_surface_target(window_handle)?;
            // SAFETY: the caller guarantees the view is valid and outlives the surface.
            let created: Result<Surface<'static>, CreateSurfaceError> =
                unsafe { instance.create_surface_unsafe(target) };
            let surface: Surface<'static> = created.map_err(|error| {
                AppError::new(&format!("creating a surface from the window handle failed: {error}"))
            })?;

            Ok(WgpuSurface { inner: surface })
        }
    }

    fn get_surface_target(window_handle: WindowHandle) -> Result<SurfaceTargetUnsafe, AppError> {
        let (raw_window_handle, raw_display_handle): (RawWindowHandle, RawDisplayHandle) = match window_handle {
            WindowHandle::UiKit { ui_view_address } => {
                let ui_view: NonNull<c_void> = NonNull::new(ui_view_address as *mut c_void)
                    .ok_or_else(|| AppError::new("the UIKit view address is null"))?;

                (
                    RawWindowHandle::UiKit(UiKitWindowHandle::new(ui_view)),
                    RawDisplayHandle::UiKit(UiKitDisplayHandle::new()),
                )
            }
        };

        Ok(SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(raw_display_handle),
            raw_window_handle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_surface_format_prefers_the_first_format_without_srgb_encoding() {
        let formats: [TextureFormat; 3] = [
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Rgba8Unorm,
        ];

        assert_eq!(choose_surface_format(&formats), Some(TextureFormat::Bgra8Unorm));
    }

    #[test]
    fn choose_surface_format_falls_back_to_the_first_format() {
        let formats: [TextureFormat; 2] = [TextureFormat::Rgba8UnormSrgb, TextureFormat::Bgra8UnormSrgb];

        assert_eq!(choose_surface_format(&formats), Some(TextureFormat::Rgba8UnormSrgb));
    }

    #[test]
    fn choose_surface_format_returns_none_for_no_formats() {
        assert_eq!(choose_surface_format(&[]), None);
    }
}
