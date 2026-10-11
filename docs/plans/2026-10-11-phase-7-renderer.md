# Phase 7: renderer implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the wgpu renderer in `shared/src/render/` behind the `render` feature: the surface, the cell and shape pipelines with their WGSL, the GPU-free scene construction `RenderScene::from_state` over a `GameState` and phase 6's `InterpolationFrame`, and the camera; proven by the `RenderScene` unit tests and by the ignored test which builds every pipeline on a headless device.

**Architecture:** `docs/architecture/client-web.md` (renderer). `RenderScene::from_state` turns a game state into instance lists in world pixels: a `TickLayer` (world shadow, world, fields, secretions, organism cells), which changes only when a tick is applied, and a `FrameLayer` (interpolated spores and shots, crosshair bars), which changes every frame, plus the `Camera`. `GameRenderer` creates the surface before a compatible adapter, builds two instanced pipelines (cells and shapes, each a triangle strip per instance with signed-distance antialiasing in WGSL), keeps the tick layer's instance buffers until the next upload, and per frame writes only the frame layer and the camera uniform before drawing tick shapes, tick cells, projectiles and the crosshair.

**Tech Stack:** Rust 1.95 (edition 2024); wgpu 30, bytemuck 1.25, raw-window-handle 0.6 (targets other than wasm32) and web-sys `HtmlCanvasElement` (wasm32), all already declared by `shared`'s `render` feature; no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `renderer`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `renderer`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "replace everything above `#[cfg(test)]`" (the file's one test module) with the given code, followed by one blank line;
  - "append inside `mod tests`, directly above its closing brace", which is the last line of the file, after one blank line separating it from the item before.
- A new file's failing test is the file holding only its `#[cfg(test)] mod tests` block (together with its `pub mod` line); the implementation step then replaces everything above `#[cfg(test)]`, which is nothing yet, with the code.
- `shared/src/render/mod.rs` is given whole each time it changes.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed while writing this plan, by executing it end to end in a scratch clone of this branch; test counts and filtered counts assume the tasks are done in order with the code exactly as given. The `finished in` timing of each `test result` line is left out. A compile failure is quoted by its first `error` line; later lines differ only in the names they report.
- The shader test (Tasks 7 and 12) needs a GPU adapter, which every Mac has (Metal). If `create_headless_device` panics on `unwrap()` of the adapter request, the command ran without access to the GPU (for example in a sandbox): run `./scripts/test/test-shaders.sh` from a script in `/tmp`, in the foreground.
- `shared/tests/forbidden_operations.rs` exempts `render/`, so the renderer may use std float functions (`f64::powi` in a scene test); it still may not name `bitcode`.

## Decisions taken where the design is silent or contradictory

- `RenderScene::from_state` takes a fourth argument, `crosshair: Option<InterpolatedPoint>`: the original drew the crosshair only for a playing member (`HUD.render`) and in the tutorial, and the camera follows the crosshair online but is fixed in the title scene and the tutorial, so neither the camera nor the state can tell. Choosing the crosshair and the camera per scene is the phase 8 driver's.
- Two layers: "rebuilt only when a tick is applied" covers the `TickLayer`; the crosshair moves every frame, so its bars join the interpolated projectiles in the `FrameLayer`. `GameRenderer::upload_tick_layer` and `GameRenderer::draw_frame` make the split explicit; `RenderScene::refresh_frame` rebuilds the frame layer and camera alone.
- The backdrop is the render pass's clear colour, not a shape: a backdrop rectangle in world pixels would follow the camera and change every frame.
- Shots at 0.8 of a cell, spores at full size: the design's "spores (0.8 scale) and shots" is read against the original, whose `renderSpores` draws `shoot.spore` at `.8` and spores at full size.
- Faithful details the design leaves out, from `Abilities.js` and `World.js`: the spores and shots of Grid and None organisms are unstroked squares (`InstanceSkinKind::Solid`); secretions of a Ghost organism are 2 px rings; neutralize and toxin fields are filled grey 100 under their ring; secretion radii are `6 / cos 45° * 2.9` px for a spore and half that for a shot.
- The crosshair is white in every scene, as designed; the original's tutorial drew it in the border colour.
- Colours: the original's `config.colors.orgs.black` palette (every world is black), in `render/render_color.rs`; phase 8's HTML views can read the same table.
- `GameRenderer` does not keep the `Instance`: a wgpu surface and device hold the context they need, and nothing reads the instance again. It keeps the adapter for `adapter_info` (the backend in use, for the manual WebGPU and WebGL2 checks).
- Surface format: the first without sRGB encoding, so instance colours and blending are sRGB values as in the original's canvas; with only sRGB formats on offer the first is used and colours come out lighter.
- WGSL: `scene_common.wgsl` (camera uniform, quad corners, distances, coverage, compositing, colour accessors) is prepended to `cell.wgsl` and `shape.wgsl` with `concat!(include_str!(...))`, so the camera uniform is declared once. Antialiasing uses signed distances in world pixels scaled by the device pixel ratio; outputs are premultiplied.
- `params` encoding: skin code in bits 0 to 7, scale in thousandths of a cell in bits 16 to 31 (`CellScale`).
- No matrix crate: `world_to_view` and `view_to_clip` are axis-aligned, so `AxisAlignedTransform` (a diagonal linear map plus a translation) composes them and uploads a column-major `mat4x4<f32>`.
- `PhysicalSize` and `ViewportSize` (physical size plus device pixel ratio) live in `camera.rs`; the view size in CSS pixels is derived from them.
- Native surface: `WindowHandle::UiKit { ui_view_address }` only, for the later iOS app; `WgpuSurface::from_window_handle` and `GameRenderer::from_window_handle` are `unsafe fn`s, since the caller vouches for the view's lifetime.
- The pipeline-compile test validates WGSL on the host's backend (Metal) under the WebGL2 limits; the GLSL translation for WebGL2 is checked by the manual `?renderer=webgl2` runs of phase 8 and 9. The test polls wgpu's futures with `Waker::noop()`, since they are ready on first poll on these targets and `shared` has no async runtime.
- `scripts/test/test-shaders.sh` runs every ignored test under `render::`.

## File structure

- Modify `shared/src/lib.rs`: declares `render` behind the `render` feature.
- Create `shared/src/render/mod.rs`: declares and re-exports the render modules.
- Create `shared/src/render/render_color.rs`: `RenderColor`, the scene colours, `get_organism_color`.
- Create `shared/src/render/camera.rs`: `PhysicalSize`, `ViewportSize`, `ViewPoint`, `AxisAlignedTransform`, `CameraUniform`, `Camera`.
- Create `shared/src/render/cell_pipeline.rs`: `InstanceSkinKind`, `CellScale`, `CellInstance`, `create_cell_pipeline`.
- Create `shared/src/render/shape_pipeline.rs`: `ShapeKind`, `ShapeGeometry`, `ShapePaint`, `ShapeInstance`, `create_shape_pipeline`.
- Create `shared/src/render/scene.rs`: `RenderScene`, `TickLayer`, `FrameLayer`, the secretion and field radii.
- Create `shared/src/render/scene_common.wgsl`, `shared/src/render/cell.wgsl`, `shared/src/render/shape.wgsl`: the shaders.
- Create `shared/src/render/scene_pipelines.rs`: `ScenePipelines`, `InstancedQuadPipelineSource`, `get_device_descriptor`, `create_instanced_quad_pipeline`, the ignored pipeline-compile test.
- Create `shared/src/render/instance_buffer.rs`: `InstanceBuffer`, `get_grown_capacity`.
- Create `shared/src/render/surface.rs`: `WgpuSurface`, `ConfiguredSurface`, `choose_surface_format`, `WindowHandle` (not wasm32).
- Create `shared/src/render/game_renderer.rs`: `RendererBackendKind`, `GameRenderer`, `get_clear_color`.
- Create `scripts/test/test-shaders.sh`: runs the ignored render tests.
- Modify `docs/architecture/client-web.md`, `docs/architecture/overview.md`, `docs/architecture/testing.md`: the renderer as built.

---

### Task 1: Render module and colours

**Files:**
- Modify: `shared/src/lib.rs`
- Create: `shared/src/render/mod.rs`
- Create: `shared/src/render/render_color.rs`
- Test: inline `mod tests` in `shared/src/render/render_color.rs`

The `render` module exists only with the `render` feature, so the server never links wgpu. `RenderColor` is the `unorm8x4` colour every instance carries; the organism palette is the original's `config.colors.orgs.black` (every game world is black), the scene colours are those of `World.js`, `Abilities.js` and `HUD.js`.

- [ ] **Step 1: Write the failing test**

In `shared/src/lib.rs`, replace:

```rust
pub mod random;
pub mod replay;
```

with:

```rust
pub mod random;
// The server builds shared without `render`, so it never links wgpu.
#[cfg(feature = "render")]
pub mod render;
pub mod replay;
```

Create `shared/src/render/mod.rs`:

```rust
pub mod render_color;

pub use render_color::*;
```

Create `shared/src/render/render_color.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::member;

    #[test]
    fn get_organism_color_gives_the_original_palette_in_palette_order() {
        let organism_colors: Vec<RenderColor> =
            member::ORGANISM_COLOR_ORDER.into_iter().map(get_organism_color).collect();

        assert_eq!(
            organism_colors,
            vec![
                RenderColor::opaque(255, 90, 81),
                RenderColor::opaque(232, 183, 155),
                RenderColor::opaque(232, 145, 95),
                RenderColor::opaque(255, 246, 86),
                RenderColor::opaque(125, 255, 200),
                RenderColor::opaque(57, 249, 86),
                RenderColor::opaque(48, 210, 255),
                RenderColor::opaque(142, 182, 255),
                RenderColor::opaque(102, 136, 244),
                RenderColor::opaque(175, 132, 255),
                RenderColor::opaque(250, 122, 255),
                RenderColor::opaque(232, 2, 216),
            ],
        );
    }

    #[test]
    fn darkened_lowers_every_colour_channel_and_keeps_the_alpha() {
        assert_eq!(SHADOW_COLOR, RenderColor::opaque(50, 50, 50));
        assert_eq!(
            RenderColor::opaque(10, 200, 30).darkened(20),
            RenderColor::opaque(0, 180, 10),
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::render_color`

Expected: compile failure, first line ``error[E0425]: cannot find type `RenderColor` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/render_color.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use crate::member::OrganismColorKind;

pub const BACKDROP_COLOR: RenderColor = RenderColor::opaque(70, 70, 70);
pub const SHADOW_COLOR: RenderColor = BACKDROP_COLOR.darkened(20);
pub const WORLD_BODY_COLOR: RenderColor = RenderColor::opaque(0, 0, 0);
pub const WORLD_BORDER_COLOR: RenderColor = RenderColor::opaque(210, 210, 210);
/// Neutralize and toxin fields.
pub const FIELD_FILL_COLOR: RenderColor = RenderColor::opaque(100, 100, 100);
pub const TOXIN_RING_COLOR: RenderColor = RenderColor::opaque(255, 111, 92);
pub const NEUTRALIZE_RING_COLOR: RenderColor = RenderColor::opaque(0, 179, 12);
pub const CROSSHAIR_COLOR: RenderColor = RenderColor::opaque(255, 255, 255);
pub const TRANSPARENT_COLOR: RenderColor = RenderColor {
    red: 0,
    green: 0,
    blue: 0,
    alpha: 0,
};

/// Channels as the shaders read them: `unorm8x4`, red first.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RenderColor {
    pub const fn opaque(red: u8, green: u8, blue: u8) -> RenderColor {
        RenderColor {
            red,
            green,
            blue,
            alpha: u8::MAX,
        }
    }

    pub const fn darkened(self, amount: u8) -> RenderColor {
        RenderColor {
            red: self.red.saturating_sub(amount),
            green: self.green.saturating_sub(amount),
            blue: self.blue.saturating_sub(amount),
            alpha: self.alpha,
        }
    }
}

/// The original's palette for its black world.
pub fn get_organism_color(color: OrganismColorKind) -> RenderColor {
    match color {
        OrganismColorKind::Fire => RenderColor::opaque(255, 90, 81),
        OrganismColorKind::Camel => RenderColor::opaque(232, 183, 155),
        OrganismColorKind::Clay => RenderColor::opaque(232, 145, 95),
        OrganismColorKind::Sun => RenderColor::opaque(255, 246, 86),
        OrganismColorKind::Leaf => RenderColor::opaque(125, 255, 200),
        OrganismColorKind::Lime => RenderColor::opaque(57, 249, 86),
        OrganismColorKind::Sky => RenderColor::opaque(48, 210, 255),
        OrganismColorKind::Lake => RenderColor::opaque(142, 182, 255),
        OrganismColorKind::Ocean => RenderColor::opaque(102, 136, 244),
        OrganismColorKind::Royal => RenderColor::opaque(175, 132, 255),
        OrganismColorKind::Petal => RenderColor::opaque(250, 122, 255),
        OrganismColorKind::Hot => RenderColor::opaque(232, 2, 216),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::render_color`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 462 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/lib.rs shared/src/render/mod.rs shared/src/render/render_color.rs
git commit -m "render module and colours"
```

### Task 2: Camera

**Files:**
- Create: `shared/src/render/camera.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/camera.rs`

`Camera` maps world pixels to clip space: `world_to_view` translates the view origin (the crosshair minus half the view for an online game, minus the 25 px margin for a local game) to the top left, `view_to_clip` is the orthographic map over the view size in CSS pixels, which is the physical size over the device pixel ratio. Both are axis-aligned (a diagonal linear map plus a translation), so `AxisAlignedTransform` composes them without a matrix crate; `CameraUniform` carries `world_to_clip` as a column-major `mat4x4<f32>` and the physical pixels per world pixel for antialiasing. `get_view_point` is for the name labels of phase 8. Test values use a 1024 by 512 physical viewport at ratio 2, so every expected float is exact.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod render_color;

pub use camera::*;
pub use render_color::*;
```

Create `shared/src/render/camera.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// 512 by 256 view pixels.
    const VIEWPORT: ViewportSize = ViewportSize {
        physical_size: PhysicalSize {
            width_pixels: 1024,
            height_pixels: 512,
        },
        device_pixel_ratio: 2.0,
    };

    #[test]
    fn view_width_pixels_divides_the_physical_size_by_the_device_pixel_ratio() {
        assert_eq!(
            (VIEWPORT.view_width_pixels(), VIEWPORT.view_height_pixels()),
            (512.0, 256.0),
        );
    }

    #[test]
    fn at_least_one_pixel_raises_only_zero_dimensions() {
        assert_eq!(
            PhysicalSize {
                width_pixels: 0,
                height_pixels: 7,
            }
            .at_least_one_pixel(),
            PhysicalSize {
                width_pixels: 1,
                height_pixels: 7,
            },
        );
    }

    #[test]
    fn centred_on_puts_the_world_point_at_the_view_centre() {
        let camera: Camera = Camera::centred_on(InterpolatedPoint { x: 1000.5, y: -40.0 }, VIEWPORT);

        assert_eq!(
            camera.get_view_point(InterpolatedPoint { x: 1000.5, y: -40.0 }),
            ViewPoint { x: 256.0, y: 128.0 },
        );
        assert_eq!(camera.view_origin, InterpolatedPoint { x: 744.5, y: -168.0 });
    }

    #[test]
    fn for_local_game_puts_the_world_origin_at_the_margin() {
        let camera: Camera = Camera::for_local_game(VIEWPORT);

        assert_eq!(
            camera.get_view_point(InterpolatedPoint { x: 0.0, y: 0.0 }),
            ViewPoint { x: 25.0, y: 25.0 },
        );
    }

    #[test]
    fn world_to_clip_maps_the_view_corners_to_the_clip_corners() {
        let camera: Camera = Camera::centred_on(InterpolatedPoint { x: 300.0, y: 200.0 }, VIEWPORT);
        let world_to_clip: AxisAlignedTransform = camera.world_to_clip();

        assert_eq!(world_to_clip.apply(44.0, 72.0), (-1.0, 1.0));
        assert_eq!(world_to_clip.apply(556.0, 328.0), (1.0, -1.0));
        assert_eq!(world_to_clip.apply(300.0, 200.0), (0.0, 0.0));
    }

    #[test]
    fn then_applies_the_first_transform_first() {
        let doubling: AxisAlignedTransform = AxisAlignedTransform {
            scale_x: 2.0,
            scale_y: 2.0,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let shifting: AxisAlignedTransform = AxisAlignedTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 3.0,
            offset_y: -1.0,
        };

        assert_eq!(doubling.then(shifting).apply(5.0, 5.0), (13.0, 9.0));
        assert_eq!(shifting.then(doubling).apply(5.0, 5.0), (16.0, 8.0));
    }

    #[test]
    fn to_uniform_holds_world_to_clip_column_major_and_the_device_pixel_ratio() {
        let camera: Camera = Camera::for_local_game(VIEWPORT);
        let camera_uniform: CameraUniform = camera.to_uniform();

        assert_eq!(
            camera_uniform.world_to_clip,
            [
                [2.0 / 512.0, 0.0, 0.0, 0.0],
                [0.0, -2.0 / 256.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [25.0 * 2.0 / 512.0 - 1.0, 1.0 - 25.0 * 2.0 / 256.0, 0.0, 1.0],
            ],
        );
        assert_eq!(camera_uniform.physical_pixels_per_world_pixel, 2.0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::camera`

Expected: compile failure, first line ``error[E0425]: cannot find type `ViewportSize` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/camera.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use crate::play::InterpolatedPoint;
use crate::play::local_game;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

impl PhysicalSize {
    /// A surface cannot be configured with a zero dimension.
    pub fn at_least_one_pixel(self) -> PhysicalSize {
        PhysicalSize {
            width_pixels: self.width_pixels.max(1),
            height_pixels: self.height_pixels.max(1),
        }
    }
}

/// One view pixel is one world pixel and `device_pixel_ratio` physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportSize {
    pub physical_size: PhysicalSize,
    pub device_pixel_ratio: f32,
}

impl ViewportSize {
    pub fn view_width_pixels(&self) -> f32 {
        self.physical_size.width_pixels as f32 / self.device_pixel_ratio
    }

    pub fn view_height_pixels(&self) -> f32 {
        self.physical_size.height_pixels as f32 / self.device_pixel_ratio
    }
}

/// View pixels from the view's top left corner, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPoint {
    pub x: f32,
    pub y: f32,
}

/// A diagonal linear map followed by a translation: `destination = scale * source + offset` on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisAlignedTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl AxisAlignedTransform {
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.scale_x * x + self.offset_x, self.scale_y * y + self.offset_y)
    }

    /// `self` first, then `next`.
    pub fn then(self, next: AxisAlignedTransform) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: next.scale_x * self.scale_x,
            scale_y: next.scale_y * self.scale_y,
            offset_x: next.scale_x * self.offset_x + next.offset_x,
            offset_y: next.scale_y * self.offset_y + next.offset_y,
        }
    }

    /// Column-major, as WGSL's `mat4x4<f32>` is laid out.
    pub fn to_column_major_matrix(self) -> [[f32; 4]; 4] {
        [
            [self.scale_x, 0.0, 0.0, 0.0],
            [0.0, self.scale_y, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [self.offset_x, self.offset_y, 0.0, 1.0],
        ]
    }
}

/// Matches `CameraUniform` in `scene_common.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub world_to_clip: [[f32; 4]; 4],
    pub physical_pixels_per_world_pixel: f32,
    pub padding: [f32; 3],
}

const _: () = assert!(std::mem::size_of::<CameraUniform>() % 16 == 0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// The world point at the view's top left corner.
    pub view_origin: InterpolatedPoint,
    pub viewport: ViewportSize,
}

impl Camera {
    pub fn centred_on(world_point: InterpolatedPoint, viewport: ViewportSize) -> Camera {
        Camera {
            view_origin: InterpolatedPoint {
                x: world_point.x - viewport.view_width_pixels() / 2.0,
                y: world_point.y - viewport.view_height_pixels() / 2.0,
            },
            viewport,
        }
    }

    /// A local game's world fills the window inside its margin.
    pub fn for_local_game(viewport: ViewportSize) -> Camera {
        let margin_pixels: f32 = local_game::LOCAL_WORLD_MARGIN_PIXELS as f32;

        Camera {
            view_origin: InterpolatedPoint {
                x: -margin_pixels,
                y: -margin_pixels,
            },
            viewport,
        }
    }

    pub fn world_to_view(&self) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: -self.view_origin.x,
            offset_y: -self.view_origin.y,
        }
    }

    /// Orthographic; clip space has y up.
    pub fn view_to_clip(&self) -> AxisAlignedTransform {
        AxisAlignedTransform {
            scale_x: 2.0 / self.viewport.view_width_pixels(),
            scale_y: -2.0 / self.viewport.view_height_pixels(),
            offset_x: -1.0,
            offset_y: 1.0,
        }
    }

    pub fn world_to_clip(&self) -> AxisAlignedTransform {
        self.world_to_view().then(self.view_to_clip())
    }

    pub fn get_view_point(&self, world_point: InterpolatedPoint) -> ViewPoint {
        let (x, y): (f32, f32) = self.world_to_view().apply(world_point.x, world_point.y);

        ViewPoint { x, y }
    }

    pub fn to_uniform(&self) -> CameraUniform {
        CameraUniform {
            world_to_clip: self.world_to_clip().to_column_major_matrix(),
            physical_pixels_per_world_pixel: self.viewport.device_pixel_ratio,
            padding: [0.0; 3],
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::camera`

Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 464 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/camera.rs
git commit -m "camera"
```

### Task 3: Cell instances

**Files:**
- Create: `shared/src/render/cell_pipeline.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/cell_pipeline.rs`

One 16 B instance per cell, spore and shot: centre, colour, and `params` with the skin code in the low byte and the scale in thousandths of a cell in the high half (`cell.wgsl` decodes it in Task 7). Projectiles have their own skin mapping: the original (`Abilities.js` `renderSpores`) drew the spores and shots of Grid and None organisms as unstroked squares, hence `InstanceSkinKind::Solid`; shots are drawn at 0.8 of a cell.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod render_color;

pub use camera::*;
pub use cell_pipeline::*;
pub use render_color::*;
```

Create `shared/src/render/cell_pipeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::member;

    #[test]
    fn new_packs_the_skin_code_and_the_scale_into_params() {
        let cell_instance: CellInstance = CellInstance::new(
            InterpolatedPoint { x: 12.5, y: -3.0 },
            RenderColor::opaque(1, 2, 3),
            InstanceSkinKind::Ghost,
            CellScale::SHOT,
        );

        assert_eq!(cell_instance.center, [12.5, -3.0]);
        assert_eq!(cell_instance.params, 2 | 800 << 16);
        assert_eq!(cell_instance.skin_code(), InstanceSkinKind::Ghost.code());
        assert_eq!(cell_instance.scale(), CellScale::SHOT);
    }

    #[test]
    fn for_cell_keeps_every_skin() {
        let instance_skins: Vec<InstanceSkinKind> =
            member::SKIN_ORDER.into_iter().map(InstanceSkinKind::for_cell).collect();

        assert_eq!(
            instance_skins,
            vec![
                InstanceSkinKind::Grid,
                InstanceSkinKind::Circles,
                InstanceSkinKind::Ghost,
                InstanceSkinKind::None,
            ],
        );
    }

    #[test]
    fn for_projectile_draws_grid_and_none_as_solid_squares() {
        let instance_skins: Vec<InstanceSkinKind> =
            member::SKIN_ORDER.into_iter().map(InstanceSkinKind::for_projectile).collect();

        assert_eq!(
            instance_skins,
            vec![
                InstanceSkinKind::Solid,
                InstanceSkinKind::Circles,
                InstanceSkinKind::Ghost,
                InstanceSkinKind::Solid,
            ],
        );
    }

    #[test]
    fn code_gives_every_skin_its_own_byte() {
        let codes: Vec<u32> = [
            InstanceSkinKind::Grid,
            InstanceSkinKind::Circles,
            InstanceSkinKind::Ghost,
            InstanceSkinKind::None,
            InstanceSkinKind::Solid,
        ]
        .into_iter()
        .map(InstanceSkinKind::code)
        .collect();

        assert_eq!(codes, vec![0, 1, 2, 3, 4]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::cell_pipeline`

Expected: compile failure, first line ``error[E0425]: cannot find type `CellInstance` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/cell_pipeline.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use crate::member::SkinKind;
use crate::play::InterpolatedPoint;
use crate::render::RenderColor;

const INSTANCE_SKIN_KIND_MASK: u32 = 0xff;
const INSTANCE_SCALE_SHIFT: u32 = 16;

/// The codes match the `INSTANCE_SKIN_*` constants of `cell.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceSkinKind {
    Grid,
    Circles,
    Ghost,
    None,
    /// A filled square without a stroke.
    Solid,
}

impl InstanceSkinKind {
    pub fn for_cell(skin: SkinKind) -> InstanceSkinKind {
        match skin {
            SkinKind::Grid => InstanceSkinKind::Grid,
            SkinKind::Circles => InstanceSkinKind::Circles,
            SkinKind::Ghost => InstanceSkinKind::Ghost,
            SkinKind::None => InstanceSkinKind::None,
        }
    }

    /// The original drew the spores and shots of every other skin as unstroked squares.
    pub fn for_projectile(skin: SkinKind) -> InstanceSkinKind {
        match skin {
            SkinKind::Circles => InstanceSkinKind::Circles,
            SkinKind::Ghost => InstanceSkinKind::Ghost,
            SkinKind::Grid | SkinKind::None => InstanceSkinKind::Solid,
        }
    }

    pub fn code(self) -> u32 {
        match self {
            InstanceSkinKind::Grid => 0,
            InstanceSkinKind::Circles => 1,
            InstanceSkinKind::Ghost => 2,
            InstanceSkinKind::None => 3,
            InstanceSkinKind::Solid => 4,
        }
    }
}

/// Thousandths of a cell width; `cell.wgsl`'s `instance_scale` divides by `INSTANCE_SCALE_UNITS_PER_CELL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellScale(pub u16);

impl CellScale {
    pub const FULL: CellScale = CellScale(1000);
    /// The original drew shots at 0.8 of a cell.
    pub const SHOT: CellScale = CellScale(800);
}

/// Field order and formats match `CELL_INSTANCE_ATTRIBUTES` and `CellInstanceInput` in `cell.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    /// World pixels.
    pub center: [f32; 2],
    pub color: RenderColor,
    /// The skin code in the low byte, the scale in the high half.
    pub params: u32,
}

const _: () = assert!(std::mem::size_of::<CellInstance>() == 16);

impl CellInstance {
    pub fn new(
        center: InterpolatedPoint,
        color: RenderColor,
        skin: InstanceSkinKind,
        scale: CellScale,
    ) -> CellInstance {
        CellInstance {
            center: [center.x, center.y],
            color,
            params: skin.code() | u32::from(scale.0) << INSTANCE_SCALE_SHIFT,
        }
    }

    pub fn skin_code(&self) -> u32 {
        self.params & INSTANCE_SKIN_KIND_MASK
    }

    pub fn scale(&self) -> CellScale {
        CellScale((self.params >> INSTANCE_SCALE_SHIFT) as u16)
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::cell_pipeline`

Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 471 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/cell_pipeline.rs
git commit -m "cell instances"
```

### Task 4: Shape instances

**Files:**
- Create: `shared/src/render/shape_pipeline.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/shape_pipeline.rs`

One 32 B instance per rectangle or ellipse: centre, half extent, fill and stroke colours, stroke width and shape kind. A transparent colour leaves the fill or the stroke out; the stroke is centred on the edge and drawn over the fill, as p5 drew them.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod render_color;
pub mod shape_pipeline;

pub use camera::*;
pub use cell_pipeline::*;
pub use render_color::*;
pub use shape_pipeline::*;
```

Create `shared/src/render/shape_pipeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_copies_the_geometry_and_the_paint() {
        let shape_instance: ShapeInstance = ShapeInstance::new(
            ShapeGeometry {
                kind: ShapeKind::Ellipse,
                center: InterpolatedPoint { x: 4.0, y: -2.5 },
                half_width_pixels: 60.0,
                half_height_pixels: 30.0,
            },
            ShapePaint::fill_and_stroke(RenderColor::opaque(1, 2, 3), RenderColor::opaque(4, 5, 6), 3.0),
        );

        assert_eq!(
            shape_instance,
            ShapeInstance {
                center: [4.0, -2.5],
                half_extent: [60.0, 30.0],
                fill_color: RenderColor::opaque(1, 2, 3),
                stroke_color: RenderColor::opaque(4, 5, 6),
                stroke_width_pixels: 3.0,
                shape_kind: 1,
            },
        );
    }

    #[test]
    fn fill_and_stroke_leave_the_other_paint_transparent() {
        assert_eq!(
            ShapePaint::fill(RenderColor::opaque(9, 9, 9)),
            ShapePaint {
                fill: RenderColor::opaque(9, 9, 9),
                stroke: render_color::TRANSPARENT_COLOR,
                stroke_width_pixels: 0.0,
            },
        );
        assert_eq!(
            ShapePaint::stroke(RenderColor::opaque(9, 9, 9), 2.0),
            ShapePaint {
                fill: render_color::TRANSPARENT_COLOR,
                stroke: RenderColor::opaque(9, 9, 9),
                stroke_width_pixels: 2.0,
            },
        );
    }

    #[test]
    fn code_gives_every_shape_kind_its_own_value() {
        assert_eq!((ShapeKind::Rectangle.code(), ShapeKind::Ellipse.code()), (0, 1));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::shape_pipeline`

Expected: compile failure, first line ``error[E0425]: cannot find type `ShapeInstance` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/shape_pipeline.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use crate::play::InterpolatedPoint;
use crate::render::RenderColor;
use crate::render::render_color;

/// The codes match the `SHAPE_KIND_*` constants of `shape.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl ShapeKind {
    pub fn code(self) -> u32 {
        match self {
            ShapeKind::Rectangle => 0,
            ShapeKind::Ellipse => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeGeometry {
    pub kind: ShapeKind,
    /// World pixels.
    pub center: InterpolatedPoint,
    pub half_width_pixels: f32,
    pub half_height_pixels: f32,
}

/// The stroke is centred on the shape's edge and drawn over the fill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapePaint {
    pub fill: RenderColor,
    pub stroke: RenderColor,
    pub stroke_width_pixels: f32,
}

impl ShapePaint {
    pub const fn fill(fill: RenderColor) -> ShapePaint {
        ShapePaint {
            fill,
            stroke: render_color::TRANSPARENT_COLOR,
            stroke_width_pixels: 0.0,
        }
    }

    pub const fn stroke(stroke: RenderColor, stroke_width_pixels: f32) -> ShapePaint {
        ShapePaint {
            fill: render_color::TRANSPARENT_COLOR,
            stroke,
            stroke_width_pixels,
        }
    }

    pub const fn fill_and_stroke(fill: RenderColor, stroke: RenderColor, stroke_width_pixels: f32) -> ShapePaint {
        ShapePaint {
            fill,
            stroke,
            stroke_width_pixels,
        }
    }
}

/// Field order and formats match `SHAPE_INSTANCE_ATTRIBUTES` and `ShapeInstanceInput` in `shape.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeInstance {
    /// World pixels.
    pub center: [f32; 2],
    pub half_extent: [f32; 2],
    pub fill_color: RenderColor,
    pub stroke_color: RenderColor,
    pub stroke_width_pixels: f32,
    pub shape_kind: u32,
}

const _: () = assert!(std::mem::size_of::<ShapeInstance>() == 32);

impl ShapeInstance {
    pub fn new(geometry: ShapeGeometry, paint: ShapePaint) -> ShapeInstance {
        ShapeInstance {
            center: [geometry.center.x, geometry.center.y],
            half_extent: [geometry.half_width_pixels, geometry.half_height_pixels],
            fill_color: paint.fill,
            stroke_color: paint.stroke,
            stroke_width_pixels: paint.stroke_width_pixels,
            shape_kind: geometry.kind.code(),
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::shape_pipeline`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 475 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/shape_pipeline.rs
git commit -m "shape instances"
```

### Task 5: Scene tick layer

**Files:**
- Create: `shared/src/render/scene.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/scene.rs`

The tick layer holds every instance which changes only when a tick is applied, in the original's draw order (`Connection.js` and `World.js`): the world shadow (backdrop minus 20, offset (+7, +6) for a rectangle and (+5, +4) for an ellipse), the world body with its 1 px border, every toxin field, every secretion, every neutralize field (grey 100 fill with a 3 px ring, as `fill(100)` in `Abilities.js`), then every organism cell, each pass in member id order. Secretions are discs in the owner colour, a 2 px ring for Ghost; their radii are the original's (`cell_width / cos 45° * 2.9`, half that for a shot), which a test checks against the simulation's squared radii. Members without an organism draw nothing.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod render_color;
pub mod scene;
pub mod shape_pipeline;

pub use camera::*;
pub use cell_pipeline::*;
pub use render_color::*;
pub use scene::*;
pub use shape_pipeline::*;
```

Create `shared/src/render/scene.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, Projectile, ThirdAbilityKind, ability_constants};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector};
    use crate::member::{Member, MemberId, OrganismColorKind};

    const OCEAN_COLOR: RenderColor = RenderColor::opaque(102, 136, 244);
    /// At (10, 0) px; at (9.5, 1) px half a tick earlier.
    const PROJECTILE: Projectile = Projectile {
        position: SubpixelPoint { x: 10_240, y: 0 },
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };

    fn create_state_with_one_organism() -> GameState {
        test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }])
    }

    fn get_member_mut(state: &mut GameState, member_id: MemberId) -> &mut Member {
        state.members.get_mut(&member_id).unwrap()
    }

    fn create_circle(center: InterpolatedPoint, radius_pixels: f32, paint: ShapePaint) -> ShapeInstance {
        ShapeInstance::new(get_circle_geometry(center, radius_pixels), paint)
    }

    #[test]
    fn tick_layer_from_state_draws_the_shadow_then_the_world_of_a_rectangle_world() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer,
            TickLayer {
                background_shapes: vec![
                    ShapeInstance {
                        center: [407.0, 406.0],
                        half_extent: [400.0, 400.0],
                        fill_color: RenderColor::opaque(50, 50, 50),
                        stroke_color: render_color::TRANSPARENT_COLOR,
                        stroke_width_pixels: 0.0,
                        shape_kind: ShapeKind::Rectangle.code(),
                    },
                    ShapeInstance {
                        center: [400.0, 400.0],
                        half_extent: [400.0, 400.0],
                        fill_color: RenderColor::opaque(0, 0, 0),
                        stroke_color: RenderColor::opaque(210, 210, 210),
                        stroke_width_pixels: 1.0,
                        shape_kind: ShapeKind::Rectangle.code(),
                    },
                ],
                organism_cells: Vec::new(),
            },
        );
    }

    #[test]
    fn tick_layer_from_state_offsets_the_shadow_of_an_ellipse_world_less() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);

        let tick_layer: TickLayer = TickLayer::from_state(&state);
        let shadow: ShapeInstance = tick_layer.background_shapes[0];

        assert_eq!(shadow.center, [405.0, 404.0]);
        assert_eq!(shadow.shape_kind, ShapeKind::Ellipse.code());
        assert_eq!(tick_layer.background_shapes[1].shape_kind, ShapeKind::Ellipse.code());
    }

    #[test]
    fn tick_layer_from_state_follows_the_shrunk_world_bounds() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.world.bounds.left = Subpixels(512);
        state.world.bounds.width = Subpixels(799 * 1024);

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(tick_layer.background_shapes[1].center, [400.0, 400.0]);
        assert_eq!(tick_layer.background_shapes[1].half_extent, [399.5, 400.0]);
    }

    #[test]
    fn tick_layer_from_state_draws_every_cell_in_the_loadout_appearance() {
        let mut state: GameState = create_state_with_one_organism();
        test_fixture::get_organism_mut(&mut state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer.organism_cells,
            vec![
                CellInstance::new(
                    InterpolatedPoint { x: 100.0, y: 100.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Circles,
                    CellScale::FULL,
                ),
                CellInstance::new(
                    InterpolatedPoint { x: 106.0, y: 100.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Circles,
                    CellScale::FULL,
                ),
            ],
        );
    }

    #[test]
    fn tick_layer_from_state_skips_members_without_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(MemberId(0), test_fixture::create_participant(MemberId(0)));
        state.members.insert(MemberId(1), test_fixture::create_spectator(MemberId(1)));

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(tick_layer.background_shapes.len(), 2);
        assert_eq!(tick_layer.organism_cells, Vec::new());
    }

    #[test]
    fn tick_layer_from_state_draws_toxin_fields_then_secretions_then_neutralize_fields() {
        let mut state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            800,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        let neutralizer: &mut Member = get_member_mut(&mut state, MemberId(0));
        let neutralizer_organism: &mut Organism = neutralizer.organism.as_mut().unwrap();
        neutralizer_organism.abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        neutralizer_organism.abilities.third_center = Some(WorldPoint { x: 100, y: 100 });
        let poisoner: &mut Member = get_member_mut(&mut state, MemberId(1));
        poisoner.loadout.as_mut().unwrap().third = ThirdAbilityKind::Toxin;
        poisoner.loadout.as_mut().unwrap().appearance.color = OrganismColorKind::Fire;
        let poisoner_organism: &mut Organism = poisoner.organism.as_mut().unwrap();
        poisoner_organism.abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        poisoner_organism.abilities.third_center = Some(WorldPoint { x: 300, y: 300 });
        poisoner_organism.abilities.spore = SporePhase::Secreting {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };
        poisoner_organism.abilities.shots[1] = ShotPhase::Secreting {
            ends_at: Tick(9),
            center: SubpixelPoint { x: 2048, y: 3072 },
        };

        let tick_layer: TickLayer = TickLayer::from_state(&state);
        let fire_color: RenderColor = RenderColor::opaque(255, 90, 81);

        assert_eq!(
            tick_layer.background_shapes[2..],
            [
                create_circle(
                    InterpolatedPoint { x: 300.0, y: 300.0 },
                    60.0,
                    ShapePaint::fill_and_stroke(
                        RenderColor::opaque(100, 100, 100),
                        RenderColor::opaque(255, 111, 92),
                        3.0,
                    ),
                ),
                create_circle(
                    InterpolatedPoint { x: 10.0, y: 0.0 },
                    SPORE_SECRETION_RADIUS_PIXELS,
                    ShapePaint::fill(fire_color),
                ),
                create_circle(
                    InterpolatedPoint { x: 2.0, y: 3.0 },
                    SHOT_SECRETION_RADIUS_PIXELS,
                    ShapePaint::fill(fire_color),
                ),
                create_circle(
                    InterpolatedPoint { x: 100.0, y: 100.0 },
                    60.0,
                    ShapePaint::fill_and_stroke(
                        RenderColor::opaque(100, 100, 100),
                        RenderColor::opaque(0, 179, 12),
                        3.0,
                    ),
                ),
            ],
        );
    }

    #[test]
    fn tick_layer_from_state_strokes_the_secretions_of_a_ghost_skin() {
        let mut state: GameState = create_state_with_one_organism();
        let member: &mut Member = get_member_mut(&mut state, MemberId(0));
        member.loadout.as_mut().unwrap().appearance.skin = SkinKind::Ghost;
        member.organism.as_mut().unwrap().abilities.spore = SporePhase::Secreting {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };

        let tick_layer: TickLayer = TickLayer::from_state(&state);

        assert_eq!(
            tick_layer.background_shapes[2..],
            [create_circle(
                InterpolatedPoint { x: 10.0, y: 0.0 },
                SPORE_SECRETION_RADIUS_PIXELS,
                ShapePaint::stroke(OCEAN_COLOR, 2.0),
            )],
        );
    }

    #[test]
    fn secretion_and_field_radii_match_the_simulation_radii() {
        let subpixels_per_pixel: f64 = f64::from(geometry::SUBPIXELS_PER_PIXEL);
        let spore_radius_squared_subpixels: f64 =
            (f64::from(SPORE_SECRETION_RADIUS_PIXELS) * subpixels_per_pixel).powi(2);
        let shot_radius_squared_subpixels: f64 =
            (f64::from(SHOT_SECRETION_RADIUS_PIXELS) * subpixels_per_pixel).powi(2);

        assert!(
            (spore_radius_squared_subpixels / ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS as f64 - 1.0)
                .abs()
                < 1e-6
        );
        assert!(
            (shot_radius_squared_subpixels / ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS as f64 - 1.0)
                .abs()
                < 1e-6
        );
        assert_eq!(
            (FIELD_RADIUS_PIXELS * FIELD_RADIUS_PIXELS) as i64,
            ability_constants::FIELD_RADIUS_SQUARED_PIXELS,
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::scene`

Expected: compile failure, first line ``error[E0425]: cannot find type `RenderColor` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/scene.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use std::f32::consts::SQRT_2;

use crate::ability::{Loadout, ShotPhase, SporePhase};
use crate::game::GameState;
use crate::geometry;
use crate::geometry::{Subpixels, WorldPoint};
use crate::member::SkinKind;
use crate::organism::Organism;
use crate::play::InterpolatedPoint;
use crate::render::render_color;
use crate::render::{
    CellInstance, CellScale, InstanceSkinKind, RenderColor, ShapeGeometry, ShapeInstance, ShapeKind, ShapePaint,
};
use crate::world::{World, WorldShapeKind};

/// The original's `cell_width / cos 45° * 2.9`.
pub const SPORE_SECRETION_RADIUS_PIXELS: f32 = geometry::CELL_WIDTH_PIXELS as f32 * SQRT_2 * 2.9;
/// Half a spore's.
pub const SHOT_SECRETION_RADIUS_PIXELS: f32 = SPORE_SECRETION_RADIUS_PIXELS / 2.0;
/// The square root of `ability_constants::FIELD_RADIUS_SQUARED_PIXELS`.
pub const FIELD_RADIUS_PIXELS: f32 = 60.0;
const FIELD_RING_WIDTH_PIXELS: f32 = 3.0;
const WORLD_BORDER_WIDTH_PIXELS: f32 = 1.0;
const GHOST_SECRETION_RING_WIDTH_PIXELS: f32 = 2.0;
const RECTANGLE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 7.0, y: 6.0 };
const ELLIPSE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 5.0, y: 4.0 };

/// Instances which change only when a tick is applied; drawn under the frame layer.
#[derive(Clone, Debug, PartialEq)]
pub struct TickLayer {
    /// In draw order: world shadow, world, toxin fields, secretions, neutralize fields.
    pub background_shapes: Vec<ShapeInstance>,
    pub organism_cells: Vec<CellInstance>,
}

impl TickLayer {
    pub fn from_state(state: &GameState) -> TickLayer {
        let rendered_organisms: Vec<RenderedOrganism> = get_rendered_organisms(state);
        let mut background_shapes: Vec<ShapeInstance> = get_world_shapes(&state.world);

        background_shapes.extend(rendered_organisms.iter().filter_map(RenderedOrganism::get_toxin_field_shape));
        background_shapes.extend(rendered_organisms.iter().flat_map(RenderedOrganism::get_secretion_shapes));
        background_shapes.extend(rendered_organisms.iter().filter_map(RenderedOrganism::get_neutralize_field_shape));

        let organism_cells: Vec<CellInstance> =
            rendered_organisms.iter().flat_map(RenderedOrganism::get_cell_instances).collect();

        TickLayer {
            background_shapes,
            organism_cells,
        }
    }
}

/// An organism with the loadout it is drawn in.
struct RenderedOrganism<'state> {
    organism: &'state Organism,
    loadout: &'state Loadout,
}

impl RenderedOrganism<'_> {
    fn color(&self) -> RenderColor {
        render_color::get_organism_color(self.loadout.appearance.color)
    }

    fn skin(&self) -> SkinKind {
        self.loadout.appearance.skin
    }

    fn get_toxin_field_shape(&self) -> Option<ShapeInstance> {
        let field_center: WorldPoint = self.organism.abilities.get_toxin_field_center(self.loadout)?;

        Some(get_field_shape(field_center, render_color::TOXIN_RING_COLOR))
    }

    fn get_neutralize_field_shape(&self) -> Option<ShapeInstance> {
        let field_center: WorldPoint = self.organism.abilities.get_neutralize_field_center(self.loadout)?;

        Some(get_field_shape(field_center, render_color::NEUTRALIZE_RING_COLOR))
    }

    /// Spore secretions, then shot secretions in slot order.
    fn get_secretion_shapes(&self) -> Vec<ShapeInstance> {
        let mut secretion_shapes: Vec<ShapeInstance> = Vec::new();

        if let SporePhase::Secreting { spores, .. } = &self.organism.abilities.spore {
            secretion_shapes.extend(spores.iter().map(|spore| {
                self.get_secretion_shape(
                    InterpolatedPoint::from_subpixel_point(spore.position),
                    SPORE_SECRETION_RADIUS_PIXELS,
                )
            }));
        }

        for shot_phase in &self.organism.abilities.shots {
            if let ShotPhase::Secreting { center, .. } = shot_phase {
                secretion_shapes.push(self.get_secretion_shape(
                    InterpolatedPoint::from_subpixel_point(*center),
                    SHOT_SECRETION_RADIUS_PIXELS,
                ));
            }
        }

        secretion_shapes
    }

    fn get_secretion_shape(&self, center: InterpolatedPoint, radius_pixels: f32) -> ShapeInstance {
        let paint: ShapePaint = match self.skin() {
            SkinKind::Ghost => ShapePaint::stroke(self.color(), GHOST_SECRETION_RING_WIDTH_PIXELS),
            SkinKind::Grid | SkinKind::Circles | SkinKind::None => ShapePaint::fill(self.color()),
        };

        ShapeInstance::new(get_circle_geometry(center, radius_pixels), paint)
    }

    fn get_cell_instances(&self) -> Vec<CellInstance> {
        let skin: InstanceSkinKind = InstanceSkinKind::for_cell(self.skin());

        self.organism
            .cells
            .iter()
            .map(|lattice_coordinate| {
                CellInstance::new(
                    get_pixel_point(self.organism.cell_center(lattice_coordinate)),
                    self.color(),
                    skin,
                    CellScale::FULL,
                )
            })
            .collect()
    }
}

/// Members with an organism, in member id order.
fn get_rendered_organisms(state: &GameState) -> Vec<RenderedOrganism<'_>> {
    state
        .members
        .values()
        .filter_map(|member| {
            Some(RenderedOrganism {
                organism: member.organism.as_ref()?,
                loadout: member.loadout.as_ref()?,
            })
        })
        .collect()
}

/// The shadow, then the world body with its border.
fn get_world_shapes(world: &World) -> Vec<ShapeInstance> {
    let half_width_pixels: f32 = get_pixels(world.bounds.width) / 2.0;
    let half_height_pixels: f32 = get_pixels(world.bounds.height) / 2.0;
    let world_center: InterpolatedPoint = InterpolatedPoint {
        x: get_pixels(world.bounds.left) + half_width_pixels,
        y: get_pixels(world.bounds.top) + half_height_pixels,
    };
    let (shape_kind, shadow_offset): (ShapeKind, InterpolatedPoint) = match world.shape {
        WorldShapeKind::Rectangle => (ShapeKind::Rectangle, RECTANGLE_WORLD_SHADOW_OFFSET),
        WorldShapeKind::Ellipse => (ShapeKind::Ellipse, ELLIPSE_WORLD_SHADOW_OFFSET),
    };
    let world_geometry: ShapeGeometry = ShapeGeometry {
        kind: shape_kind,
        center: world_center,
        half_width_pixels,
        half_height_pixels,
    };
    let shadow_geometry: ShapeGeometry = ShapeGeometry {
        kind: shape_kind,
        center: InterpolatedPoint {
            x: world_center.x + shadow_offset.x,
            y: world_center.y + shadow_offset.y,
        },
        half_width_pixels,
        half_height_pixels,
    };

    vec![
        ShapeInstance::new(shadow_geometry, ShapePaint::fill(render_color::SHADOW_COLOR)),
        ShapeInstance::new(
            world_geometry,
            ShapePaint::fill_and_stroke(
                render_color::WORLD_BODY_COLOR,
                render_color::WORLD_BORDER_COLOR,
                WORLD_BORDER_WIDTH_PIXELS,
            ),
        ),
    ]
}

fn get_field_shape(field_center: WorldPoint, ring_color: RenderColor) -> ShapeInstance {
    ShapeInstance::new(
        get_circle_geometry(get_pixel_point(field_center), FIELD_RADIUS_PIXELS),
        ShapePaint::fill_and_stroke(render_color::FIELD_FILL_COLOR, ring_color, FIELD_RING_WIDTH_PIXELS),
    )
}

fn get_circle_geometry(center: InterpolatedPoint, radius_pixels: f32) -> ShapeGeometry {
    ShapeGeometry {
        kind: ShapeKind::Ellipse,
        center,
        half_width_pixels: radius_pixels,
        half_height_pixels: radius_pixels,
    }
}

fn get_pixel_point(point: WorldPoint) -> InterpolatedPoint {
    InterpolatedPoint {
        x: point.x as f32,
        y: point.y as f32,
    }
}

fn get_pixels(subpixels: Subpixels) -> f32 {
    (f64::from(subpixels.0) / f64::from(geometry::SUBPIXELS_PER_PIXEL)) as f32
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::scene`

Expected: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 478 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/scene.rs
git commit -m "scene tick layer"
```

### Task 6: Scene frame layer

**Files:**
- Modify: `shared/src/render/scene.rs`
- Test: inline `mod tests` in `shared/src/render/scene.rs`

The frame layer holds what changes every frame: the flying spores and shots, interpolated with phase 6's `InterpolationFrame` between their previous and current tick positions (a secreting spore or shot is drawn only as its secretion), and the crosshair bars when the caller passes a crosshair. `RenderScene` bundles both layers with the camera; `refresh_frame` rebuilds only the frame layer and the camera, for a frame which applied no tick.

- [ ] **Step 1: Write the failing test**

In `shared/src/render/scene.rs`, replace:

```rust
    use crate::member::{Member, MemberId, OrganismColorKind};
```

with:

```rust
    use crate::member::{Member, MemberId, OrganismColorKind};
    use crate::render::{PhysicalSize, ViewportSize};
```

In `shared/src/render/scene.rs`, replace:

```rust
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };
```

with:

```rust
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };
    const VIEWPORT: ViewportSize = ViewportSize {
        physical_size: PhysicalSize {
            width_pixels: 1024,
            height_pixels: 512,
        },
        device_pixel_ratio: 2.0,
    };
    const HALFWAY: InterpolationFrame = InterpolationFrame { alpha: 0.5 };
```

In `shared/src/render/scene.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn frame_layer_from_state_interpolates_flying_spores_then_flying_shots() {
        let mut state: GameState = create_state_with_one_organism();
        let member: &mut Member = get_member_mut(&mut state, MemberId(0));
        member.loadout.as_mut().unwrap().appearance.skin = SkinKind::Grid;
        let organism: &mut Organism = member.organism.as_mut().unwrap();
        organism.abilities.spore = SporePhase::Flying {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };
        organism.abilities.shots[0] = ShotPhase::Flying {
            ends_at: Tick(9),
            shot: PROJECTILE,
        };

        let frame_layer: FrameLayer = FrameLayer::from_state(&state, &HALFWAY, None);

        assert_eq!(
            frame_layer.projectile_cells,
            vec![
                CellInstance::new(
                    InterpolatedPoint { x: 9.5, y: 1.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Solid,
                    CellScale::FULL,
                ),
                CellInstance::new(
                    InterpolatedPoint { x: 9.5, y: 1.0 },
                    OCEAN_COLOR,
                    InstanceSkinKind::Solid,
                    CellScale::SHOT,
                ),
            ],
        );
    }

    #[test]
    fn frame_layer_from_state_draws_no_secreting_projectile() {
        let mut state: GameState = create_state_with_one_organism();
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));
        organism.abilities.spore = SporePhase::Secreting {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };
        organism.abilities.shots[1] = ShotPhase::Secreting {
            ends_at: Tick(9),
            center: PROJECTILE.position,
        };

        let frame_layer: FrameLayer = FrameLayer::from_state(&state, &HALFWAY, None);

        assert_eq!(frame_layer.projectile_cells, Vec::new());
    }

    #[test]
    fn frame_layer_from_state_draws_the_crosshair_only_when_given() {
        let state: GameState = create_state_with_one_organism();
        let crosshair: InterpolatedPoint = InterpolatedPoint { x: 50.5, y: 60.0 };

        let shown: FrameLayer = FrameLayer::from_state(&state, &HALFWAY, Some(crosshair));
        let hidden: FrameLayer = FrameLayer::from_state(&state, &HALFWAY, None);

        assert_eq!(
            shown.crosshair_shapes,
            vec![
                ShapeInstance {
                    center: [50.5, 60.0],
                    half_extent: [4.0, 0.5],
                    fill_color: RenderColor::opaque(255, 255, 255),
                    stroke_color: render_color::TRANSPARENT_COLOR,
                    stroke_width_pixels: 0.0,
                    shape_kind: ShapeKind::Rectangle.code(),
                },
                ShapeInstance {
                    center: [50.5, 60.0],
                    half_extent: [0.5, 4.0],
                    fill_color: RenderColor::opaque(255, 255, 255),
                    stroke_color: render_color::TRANSPARENT_COLOR,
                    stroke_width_pixels: 0.0,
                    shape_kind: ShapeKind::Rectangle.code(),
                },
            ],
        );
        assert_eq!(hidden.crosshair_shapes, Vec::new());
    }

    #[test]
    fn render_scene_from_state_builds_both_layers_with_the_camera() {
        let state: GameState = create_state_with_one_organism();
        let camera: Camera = Camera::centred_on(InterpolatedPoint { x: 100.0, y: 100.0 }, VIEWPORT);

        let render_scene: RenderScene = RenderScene::from_state(&state, &HALFWAY, camera, None);

        assert_eq!(
            render_scene,
            RenderScene {
                tick_layer: TickLayer::from_state(&state),
                frame_layer: FrameLayer::from_state(&state, &HALFWAY, None),
                camera,
            },
        );
    }

    #[test]
    fn refresh_frame_replaces_the_frame_layer_and_the_camera_and_keeps_the_tick_layer() {
        let mut state: GameState = create_state_with_one_organism();
        let first_camera: Camera = Camera::centred_on(InterpolatedPoint { x: 100.0, y: 100.0 }, VIEWPORT);
        let mut render_scene: RenderScene = RenderScene::from_state(&state, &HALFWAY, first_camera, None);
        let tick_layer: TickLayer = render_scene.tick_layer.clone();
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));
        organism.cells.insert(LatticeCoordinate { i: 0, j: 1 });
        organism.abilities.spore = SporePhase::Flying {
            ends_at: Tick(9),
            spores: vec![PROJECTILE],
        };
        let second_camera: Camera = Camera::for_local_game(VIEWPORT);
        let crosshair: InterpolatedPoint = InterpolatedPoint { x: 1.0, y: 2.0 };

        render_scene.refresh_frame(&state, &HALFWAY, second_camera, Some(crosshair));

        assert_eq!(render_scene.tick_layer, tick_layer);
        assert_eq!(
            render_scene.frame_layer,
            FrameLayer::from_state(&state, &HALFWAY, Some(crosshair))
        );
        assert_eq!(render_scene.camera, second_camera);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::scene`

Expected: compile failure, first line ``error[E0425]: cannot find type `InterpolationFrame` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/scene.rs`, replace:

```rust
use crate::play::InterpolatedPoint;
```

with:

```rust
use crate::play::{InterpolatedPoint, InterpolationFrame};
```

In `shared/src/render/scene.rs`, replace:

```rust
    CellInstance, CellScale,
```

with:

```rust
    Camera, CellInstance, CellScale,
```

In `shared/src/render/scene.rs`, replace:

```rust
const ELLIPSE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 5.0, y: 4.0 };
```

with:

```rust
const ELLIPSE_WORLD_SHADOW_OFFSET: InterpolatedPoint = InterpolatedPoint { x: 5.0, y: 4.0 };
const CROSSHAIR_HALF_LENGTH_PIXELS: f32 = 4.0;
const CROSSHAIR_HALF_THICKNESS_PIXELS: f32 = 0.5;
```

In `shared/src/render/scene.rs`, replace:

```rust
/// Instances which change only when a tick is applied
```

with:

```rust
/// Everything one frame draws, in world pixels, built without a GPU.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    pub tick_layer: TickLayer,
    pub frame_layer: FrameLayer,
    pub camera: Camera,
}

impl RenderScene {
    /// `crosshair` is drawn when given.
    pub fn from_state(
        state: &GameState,
        interpolation: &InterpolationFrame,
        camera: Camera,
        crosshair: Option<InterpolatedPoint>,
    ) -> RenderScene {
        RenderScene {
            tick_layer: TickLayer::from_state(state),
            frame_layer: FrameLayer::from_state(state, interpolation, crosshair),
            camera,
        }
    }

    /// For a frame without a newly applied tick; the tick layer is kept.
    pub fn refresh_frame(
        &mut self,
        state: &GameState,
        interpolation: &InterpolationFrame,
        camera: Camera,
        crosshair: Option<InterpolatedPoint>,
    ) {
        self.frame_layer = FrameLayer::from_state(state, interpolation, crosshair);
        self.camera = camera;
    }
}

/// Instances which change only when a tick is applied
```

In `shared/src/render/scene.rs`, replace:

```rust
/// An organism with the loadout it is drawn in.
```

with:

```rust
/// Instances which change every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLayer {
    /// Per organism: flying spores, then flying shots in slot order.
    pub projectile_cells: Vec<CellInstance>,
    /// Drawn over the projectiles.
    pub crosshair_shapes: Vec<ShapeInstance>,
}

impl FrameLayer {
    pub fn from_state(
        state: &GameState,
        interpolation: &InterpolationFrame,
        crosshair: Option<InterpolatedPoint>,
    ) -> FrameLayer {
        let projectile_cells: Vec<CellInstance> = get_rendered_organisms(state)
            .iter()
            .flat_map(|rendered_organism| rendered_organism.get_projectile_instances(interpolation))
            .collect();
        let crosshair_shapes: Vec<ShapeInstance> = crosshair.map(get_crosshair_shapes).unwrap_or_default();

        FrameLayer {
            projectile_cells,
            crosshair_shapes,
        }
    }
}

/// An organism with the loadout it is drawn in.
```

In `shared/src/render/scene.rs`, replace:

```rust
                    CellScale::FULL,
                )
            })
            .collect()
    }
```

with:

```rust
                    CellScale::FULL,
                )
            })
            .collect()
    }

    fn get_projectile_instances(&self, interpolation: &InterpolationFrame) -> Vec<CellInstance> {
        let skin: InstanceSkinKind = InstanceSkinKind::for_projectile(self.skin());
        let mut projectile_instances: Vec<CellInstance> = Vec::new();

        if let SporePhase::Flying { spores, .. } = &self.organism.abilities.spore {
            projectile_instances.extend(spores.iter().map(|spore| {
                CellInstance::new(
                    interpolation.get_flying_projectile_position(spore),
                    self.color(),
                    skin,
                    CellScale::FULL,
                )
            }));
        }

        for shot_phase in &self.organism.abilities.shots {
            if let ShotPhase::Flying { shot, .. } = shot_phase {
                projectile_instances.push(CellInstance::new(
                    interpolation.get_flying_projectile_position(shot),
                    self.color(),
                    skin,
                    CellScale::SHOT,
                ));
            }
        }

        projectile_instances
    }
```

In `shared/src/render/scene.rs`, replace:

```rust
fn get_field_shape(
```

with:

```rust
/// A horizontal bar, then a vertical one.
fn get_crosshair_shapes(crosshair: InterpolatedPoint) -> Vec<ShapeInstance> {
    let bar_paint: ShapePaint = ShapePaint::fill(render_color::CROSSHAIR_COLOR);
    let horizontal_bar: ShapeGeometry = ShapeGeometry {
        kind: ShapeKind::Rectangle,
        center: crosshair,
        half_width_pixels: CROSSHAIR_HALF_LENGTH_PIXELS,
        half_height_pixels: CROSSHAIR_HALF_THICKNESS_PIXELS,
    };
    let vertical_bar: ShapeGeometry = ShapeGeometry {
        kind: ShapeKind::Rectangle,
        center: crosshair,
        half_width_pixels: CROSSHAIR_HALF_THICKNESS_PIXELS,
        half_height_pixels: CROSSHAIR_HALF_LENGTH_PIXELS,
    };

    vec![
        ShapeInstance::new(horizontal_bar, bar_paint),
        ShapeInstance::new(vertical_bar, bar_paint),
    ]
}

fn get_field_shape(
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::scene`

Expected: `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 478 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/scene.rs
git commit -m "scene frame layer"
```

### Task 7: Scene pipelines and shaders

**Files:**
- Create: `shared/src/render/scene_common.wgsl`
- Create: `shared/src/render/cell.wgsl`
- Create: `shared/src/render/shape.wgsl`
- Create: `shared/src/render/scene_pipelines.rs`
- Create: `scripts/test/test-shaders.sh`
- Modify: `shared/src/render/cell_pipeline.rs`
- Modify: `shared/src/render/shape_pipeline.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/scene_pipelines.rs` (`#[ignore]`, needs a GPU adapter)

Both pipelines draw one triangle strip of four vertices per instance, corners from `vertex_index`, with premultiplied alpha blending and no depth. `scene_common.wgsl` is prepended to `cell.wgsl` and `shape.wgsl`: the camera uniform, quad corners, signed distances to rectangles and ellipses, coverage per physical pixel (antialiasing as the original's canvas did) and stroke-over-fill compositing. Each shader names the channels of its packed data only in accessor functions (`instance_skin_kind`, `instance_scale`, `instance_color`, `get_alpha`, `get_rgb`). WGSL is validated only when a pipeline is built, so the test builds both on a headless device with `Limits::downlevel_webgl2_defaults()` for two surface formats; it is `#[ignore]` and runs through `scripts/test/test-shaders.sh`. The test polls wgpu's futures with a no-op waker, since they are ready on first poll on these targets and `shared` has no async runtime.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod render_color;
pub mod scene;
pub mod scene_pipelines;
pub mod shape_pipeline;

pub use camera::*;
pub use cell_pipeline::*;
pub use render_color::*;
pub use scene::*;
pub use scene_pipelines::*;
pub use shape_pipeline::*;
```

Create `shared/src/render/scene_pipelines.rs`:

```rust
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
        let (device, _queue): (Device, Queue) = block_on(adapter.request_device(&get_device_descriptor())).unwrap();

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
```

Create `scripts/test/test-shaders.sh`:

```sh
#!/usr/bin/env bash
# Runs the ignored test which builds every render pipeline on a headless GPU device; WGSL is validated only then.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --features render --lib render:: -- --ignored
```

- [ ] **Step 2: Make the script executable**

Run: `chmod +x scripts/test/test-shaders.sh`

Expected: no output.

- [ ] **Step 3: Run the test to verify it fails**

Run: `./scripts/test/test-shaders.sh`

Expected: compile failure, first line ``error[E0425]: cannot find type `Device` in this scope``.

- [ ] **Step 4: Write the shaders**

Create `shared/src/render/scene_common.wgsl`:

```wgsl
// Prepended to cell.wgsl and shape.wgsl. The layout of CameraUniform matches camera.rs.

struct CameraUniform {
    world_to_clip: mat4x4<f32>,
    physical_pixels_per_world_pixel: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

// One pixel of antialiasing around a quad's shape and stroke.
const QUAD_MARGIN_PIXELS: f32 = 1.0;

fn get_clip_position(world_position: vec2<f32>) -> vec4<f32> {
    return camera.world_to_clip * vec4<f32>(world_position, 0.0, 1.0);
}

// Triangle strip order: (-1, -1), (1, -1), (-1, 1), (1, 1).
fn get_quad_corner(vertex_index: u32) -> vec2<f32> {
    return vec2<f32>(f32(vertex_index & 1u), f32(vertex_index >> 1u)) * 2.0 - 1.0;
}

// The proportion of a physical pixel inside a shape, from the signed distance of its centre to the shape's edge.
fn get_coverage(signed_distance_pixels: f32) -> f32 {
    return clamp(0.5 - signed_distance_pixels * camera.physical_pixels_per_world_pixel, 0.0, 1.0);
}

// A stroke is centred on the edge.
fn get_stroke_coverage(signed_distance_pixels: f32, stroke_width_pixels: f32) -> f32 {
    let half_width_pixels: f32 = stroke_width_pixels * 0.5;

    return get_coverage(signed_distance_pixels - half_width_pixels)
        - get_coverage(signed_distance_pixels + half_width_pixels);
}

fn get_rectangle_distance(local_position: vec2<f32>, half_extent: vec2<f32>) -> f32 {
    let outside: vec2<f32> = abs(local_position) - half_extent;

    return max(outside.x, outside.y);
}

// Exact for a circle; for an ellipse, the implicit function divided by its gradient's length.
fn get_ellipse_distance(local_position: vec2<f32>, radii: vec2<f32>) -> f32 {
    let normalized_length: f32 = length(local_position / radii);
    let gradient_length: f32 = length(local_position / (radii * radii));

    if (gradient_length == 0.0) {
        return -min(radii.x, radii.y);
    }

    return normalized_length * (normalized_length - 1.0) / gradient_length;
}

fn get_alpha(color: vec4<f32>) -> f32 {
    return color.a;
}

fn get_rgb(color: vec4<f32>) -> vec3<f32> {
    return color.rgb;
}

fn get_premultiplied(color: vec4<f32>, coverage: f32) -> vec4<f32> {
    let alpha: f32 = get_alpha(color) * coverage;

    return vec4<f32>(get_rgb(color) * alpha, alpha);
}

fn get_stroke_over_fill(
    fill_color: vec4<f32>,
    fill_coverage: f32,
    stroke_color: vec4<f32>,
    stroke_coverage: f32,
) -> vec4<f32> {
    let fill: vec4<f32> = get_premultiplied(fill_color, fill_coverage);
    let stroke: vec4<f32> = get_premultiplied(stroke_color, stroke_coverage);

    return stroke + fill * (1.0 - get_alpha(stroke));
}
```

Create `shared/src/render/cell.wgsl`:

```wgsl
// The instance layout matches CellInstance in cell_pipeline.rs.

const INSTANCE_SKIN_GRID: u32 = 0u;
const INSTANCE_SKIN_CIRCLES: u32 = 1u;
const INSTANCE_SKIN_GHOST: u32 = 2u;
const INSTANCE_SKIN_NONE: u32 = 3u;
const INSTANCE_SCALE_UNITS_PER_CELL: f32 = 1000.0;
// Half of geometry::CELL_WIDTH_PIXELS.
const CELL_HALF_WIDTH_PIXELS: f32 = 3.0;
const GRID_STROKE_COLOR: vec4<f32> = vec4<f32>(40.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0);
const GRID_STROKE_WIDTH_PIXELS: f32 = 0.25;
const OUTLINE_STROKE_WIDTH_PIXELS: f32 = 1.0;

struct CellInstanceInput {
    @location(0) center: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: u32,
}

struct CellVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) half_width_pixels: f32,
    @location(3) @interpolate(flat) skin_kind: u32,
}

fn instance_skin_kind(params: u32) -> u32 {
    return params & 0xffu;
}

fn instance_scale(params: u32) -> f32 {
    return f32(params >> 16u) / INSTANCE_SCALE_UNITS_PER_CELL;
}

fn instance_color(color: vec4<f32>) -> vec4<f32> {
    return color;
}

@vertex
fn cell_vertex_main(@builtin(vertex_index) vertex_index: u32, instance: CellInstanceInput) -> CellVertexOutput {
    let half_width_pixels: f32 = CELL_HALF_WIDTH_PIXELS * instance_scale(instance.params);
    let local_position: vec2<f32> = get_quad_corner(vertex_index)
        * (half_width_pixels + OUTLINE_STROKE_WIDTH_PIXELS * 0.5 + QUAD_MARGIN_PIXELS);

    var output: CellVertexOutput;
    output.clip_position = get_clip_position(instance.center + local_position);
    output.local_position = local_position;
    output.color = instance_color(instance.color);
    output.half_width_pixels = half_width_pixels;
    output.skin_kind = instance_skin_kind(instance.params);

    return output;
}

@fragment
fn cell_fragment_main(input: CellVertexOutput) -> @location(0) vec4<f32> {
    let half_extent: vec2<f32> = vec2<f32>(input.half_width_pixels);
    let square_distance: f32 = get_rectangle_distance(input.local_position, half_extent);

    if (input.skin_kind == INSTANCE_SKIN_GRID) {
        return get_stroke_over_fill(
            input.color,
            get_coverage(square_distance),
            GRID_STROKE_COLOR,
            get_stroke_coverage(square_distance, GRID_STROKE_WIDTH_PIXELS),
        );
    }

    if (input.skin_kind == INSTANCE_SKIN_CIRCLES) {
        return get_premultiplied(input.color, get_coverage(get_ellipse_distance(input.local_position, half_extent)));
    }

    if (input.skin_kind == INSTANCE_SKIN_GHOST) {
        return get_premultiplied(input.color, get_stroke_coverage(square_distance, OUTLINE_STROKE_WIDTH_PIXELS));
    }

    if (input.skin_kind == INSTANCE_SKIN_NONE) {
        return get_stroke_over_fill(
            input.color,
            get_coverage(square_distance),
            input.color,
            get_stroke_coverage(square_distance, OUTLINE_STROKE_WIDTH_PIXELS),
        );
    }

    return get_premultiplied(input.color, get_coverage(square_distance));
}
```

Create `shared/src/render/shape.wgsl`:

```wgsl
// The instance layout matches ShapeInstance in shape_pipeline.rs.

const SHAPE_KIND_ELLIPSE: u32 = 1u;

struct ShapeInstanceInput {
    @location(0) center: vec2<f32>,
    @location(1) half_extent: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width_pixels: f32,
    @location(5) shape_kind: u32,
}

struct ShapeVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
    @location(1) half_extent: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width_pixels: f32,
    @location(5) @interpolate(flat) shape_kind: u32,
}

@vertex
fn shape_vertex_main(@builtin(vertex_index) vertex_index: u32, instance: ShapeInstanceInput) -> ShapeVertexOutput {
    let local_position: vec2<f32> = get_quad_corner(vertex_index)
        * (instance.half_extent + instance.stroke_width_pixels * 0.5 + QUAD_MARGIN_PIXELS);

    var output: ShapeVertexOutput;
    output.clip_position = get_clip_position(instance.center + local_position);
    output.local_position = local_position;
    output.half_extent = instance.half_extent;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.stroke_width_pixels = instance.stroke_width_pixels;
    output.shape_kind = instance.shape_kind;

    return output;
}

@fragment
fn shape_fragment_main(input: ShapeVertexOutput) -> @location(0) vec4<f32> {
    var signed_distance_pixels: f32 = get_rectangle_distance(input.local_position, input.half_extent);

    if (input.shape_kind == SHAPE_KIND_ELLIPSE) {
        signed_distance_pixels = get_ellipse_distance(input.local_position, input.half_extent);
    }

    return get_stroke_over_fill(
        input.fill_color,
        get_coverage(signed_distance_pixels),
        input.stroke_color,
        get_stroke_coverage(signed_distance_pixels, input.stroke_width_pixels),
    );
}
```

- [ ] **Step 5: Implement the pipelines**

In `shared/src/render/scene_pipelines.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, BufferBindingType,
    BufferSize, ColorTargetState, ColorWrites, Device, DeviceDescriptor, ErrorFilter, ErrorScopeGuard,
    ExperimentalFeatures, Features, FragmentState, FrontFace, Limits, MemoryHints, MultisampleState,
    PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, TextureFormat, Trace, VertexBufferLayout, VertexState,
};

use crate::error::AppError;
use crate::render::CameraUniform;
use crate::render::{cell_pipeline, shape_pipeline};

/// Per instance; one triangle strip.
pub const QUAD_VERTEX_COUNT: u32 = 4;

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

/// WebGL2's limits on every backend, so that what runs here runs on WebGL2.
pub fn get_device_descriptor() -> DeviceDescriptor<'static> {
    DeviceDescriptor {
        label: Some("bacter-renderer-device"),
        required_features: Features::empty(),
        required_limits: Limits::downlevel_webgl2_defaults(),
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
            binding: 0,
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
```

In `shared/src/render/cell_pipeline.rs`, replace:

```rust
use crate::member::SkinKind;
use crate::play::InterpolatedPoint;
use crate::render::RenderColor;

const INSTANCE_SKIN_KIND_MASK: u32 = 0xff;
```

with:

```rust
use wgpu::{
    BufferAddress, Device, PipelineLayout, RenderPipeline, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexStepMode,
};

use crate::member::SkinKind;
use crate::play::InterpolatedPoint;
use crate::render::scene_pipelines;
use crate::render::{InstancedQuadPipelineSource, RenderColor};

const CELL_SHADER_SOURCE: &str = concat!(include_str!("scene_common.wgsl"), include_str!("cell.wgsl"));
const CELL_INSTANCE_ATTRIBUTES: [VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4, 2 => Uint32];
const INSTANCE_SKIN_KIND_MASK: u32 = 0xff;
```

In `shared/src/render/cell_pipeline.rs`, replace:

```rust
#[cfg(test)]
mod tests {
```

with:

```rust
pub fn create_cell_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    scene_pipelines::create_instanced_quad_pipeline(
        device,
        pipeline_layout,
        surface_format,
        InstancedQuadPipelineSource {
            label: "bacter-cell-pipeline",
            shader_source: CELL_SHADER_SOURCE,
            vertex_entry_point: "cell_vertex_main",
            fragment_entry_point: "cell_fragment_main",
            instance_layout: VertexBufferLayout {
                array_stride: std::mem::size_of::<CellInstance>() as BufferAddress,
                step_mode: VertexStepMode::Instance,
                attributes: &CELL_INSTANCE_ATTRIBUTES,
            },
        },
    )
}

#[cfg(test)]
mod tests {
```

In `shared/src/render/shape_pipeline.rs`, replace:

```rust
use crate::play::InterpolatedPoint;
use crate::render::RenderColor;
use crate::render::render_color;
```

with:

```rust
use wgpu::{
    BufferAddress, Device, PipelineLayout, RenderPipeline, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexStepMode,
};

use crate::play::InterpolatedPoint;
use crate::render::{InstancedQuadPipelineSource, RenderColor};
use crate::render::{render_color, scene_pipelines};

const SHAPE_SHADER_SOURCE: &str = concat!(include_str!("scene_common.wgsl"), include_str!("shape.wgsl"));
const SHAPE_INSTANCE_ATTRIBUTES: [VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x2,
    2 => Unorm8x4,
    3 => Unorm8x4,
    4 => Float32,
    5 => Uint32,
];
```

In `shared/src/render/shape_pipeline.rs`, replace:

```rust
#[cfg(test)]
mod tests {
```

with:

```rust
pub fn create_shape_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    scene_pipelines::create_instanced_quad_pipeline(
        device,
        pipeline_layout,
        surface_format,
        InstancedQuadPipelineSource {
            label: "bacter-shape-pipeline",
            shader_source: SHAPE_SHADER_SOURCE,
            vertex_entry_point: "shape_vertex_main",
            fragment_entry_point: "shape_fragment_main",
            instance_layout: VertexBufferLayout {
                array_stride: std::mem::size_of::<ShapeInstance>() as BufferAddress,
                step_mode: VertexStepMode::Instance,
                attributes: &SHAPE_INSTANCE_ATTRIBUTES,
            },
        },
    )
}

#[cfg(test)]
mod tests {
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `./scripts/test/test-shaders.sh`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 491 filtered out`.

On a machine without a GPU adapter the test panics in `create_headless_device`; run it where one is available (any Mac).

- [ ] **Step 7: Run the render unit tests**

Run: `cargo test -p shared --features render --lib render::`

Expected: `test result: ok. 29 passed; 0 failed; 1 ignored; 0 measured; 462 filtered out`.

- [ ] **Step 8: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/scene_common.wgsl shared/src/render/cell.wgsl shared/src/render/shape.wgsl shared/src/render/scene_pipelines.rs shared/src/render/cell_pipeline.rs shared/src/render/shape_pipeline.rs scripts/test/test-shaders.sh
git commit -m "scene pipelines and shaders"
```

### Task 8: Instance buffer

**Files:**
- Create: `shared/src/render/instance_buffer.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/instance_buffer.rs`

`InstanceBuffer<T>` is a vertex buffer of `bytemuck::Pod` instances which every write replaces, growing to the next power of two when the written instances exceed its capacity. `written_slice` is `None` for nothing written, since wgpu panics on an empty buffer slice; the draw skips such a buffer. Only the growth rule is testable without a GPU.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod instance_buffer;
pub mod render_color;
pub mod scene;
pub mod scene_pipelines;
pub mod shape_pipeline;

pub use camera::*;
pub use cell_pipeline::*;
pub use instance_buffer::*;
pub use render_color::*;
pub use scene::*;
pub use scene_pipelines::*;
pub use shape_pipeline::*;
```

Create `shared/src/render/instance_buffer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_grown_capacity_keeps_a_sufficient_capacity() {
        assert_eq!(get_grown_capacity(256, 0), 256);
        assert_eq!(get_grown_capacity(256, 256), 256);
    }

    #[test]
    fn get_grown_capacity_grows_to_the_next_power_of_two() {
        assert_eq!(get_grown_capacity(256, 257), 512);
        assert_eq!(get_grown_capacity(256, 1500), 2048);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::instance_buffer`

Expected: compile failure, first line ``error[E0425]: cannot find function `get_grown_capacity` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/instance_buffer.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
use std::marker::PhantomData;

use wgpu::{Buffer, BufferAddress, BufferDescriptor, BufferSlice, BufferUsages, Device, Queue};

/// Instances a buffer holds when first created.
pub const INITIAL_INSTANCE_CAPACITY: usize = 256;

/// A vertex buffer of per-instance data which grows to hold whatever is written to it.
pub struct InstanceBuffer<T> {
    label: &'static str,
    buffer: Buffer,
    capacity: usize,
    count: u32,
    instance_type: PhantomData<T>,
}

impl<T: bytemuck::Pod> InstanceBuffer<T> {
    pub fn new(device: &Device, label: &'static str) -> InstanceBuffer<T> {
        InstanceBuffer {
            label,
            buffer: create_buffer::<T>(device, label, INITIAL_INSTANCE_CAPACITY),
            capacity: INITIAL_INSTANCE_CAPACITY,
            count: 0,
            instance_type: PhantomData,
        }
    }

    /// Replaces the contents.
    pub fn write(&mut self, device: &Device, queue: &Queue, instances: &[T]) {
        let grown_capacity: usize = get_grown_capacity(self.capacity, instances.len());

        if grown_capacity != self.capacity {
            self.buffer = create_buffer::<T>(device, self.label, grown_capacity);
            self.capacity = grown_capacity;
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(instances));
        self.count = u32::try_from(instances.len()).unwrap_or(u32::MAX);
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// `None` when nothing is written; a wgpu buffer slice cannot be empty.
    pub fn written_slice(&self) -> Option<BufferSlice<'_>> {
        if self.count == 0 {
            return None;
        }

        let written_bytes: BufferAddress = BufferAddress::from(self.count) * get_instance_size::<T>();

        Some(self.buffer.slice(..written_bytes))
    }
}

/// The current capacity while it suffices, else the next power of two at least `required`.
pub fn get_grown_capacity(capacity: usize, required: usize) -> usize {
    if required <= capacity {
        return capacity;
    }

    required.next_power_of_two()
}

fn create_buffer<T>(device: &Device, label: &'static str, capacity: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: capacity as BufferAddress * get_instance_size::<T>(),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn get_instance_size<T>() -> BufferAddress {
    std::mem::size_of::<T>() as BufferAddress
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::instance_buffer`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 492 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/instance_buffer.rs
git commit -m "instance buffer"
```

### Task 9: Surface

**Files:**
- Create: `shared/src/render/surface.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/surface.rs`

The surface is created before the adapter, since WebGL2 needs the adapter requested compatible with it: `WgpuSurface` is the unconfigured surface (`from_canvas` on wasm32, `unsafe from_window_handle` with a UIKit view elsewhere, for the later iOS app), and `configure` turns it into a `ConfiguredSurface` once the device exists. The surface format is the first without sRGB encoding, so instance colours and blending stay in sRGB values as the original's canvas kept them; the first format is the fallback. Fifo present mode, sizes clamped to at least one pixel.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod instance_buffer;
pub mod render_color;
pub mod scene;
pub mod scene_pipelines;
pub mod shape_pipeline;
pub mod surface;

pub use camera::*;
pub use cell_pipeline::*;
pub use instance_buffer::*;
pub use render_color::*;
pub use scene::*;
pub use scene_pipelines::*;
pub use shape_pipeline::*;
pub use surface::*;
```

Create `shared/src/render/surface.rs`:

```rust
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
        assert_eq!(choose_surface_format(&[]), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::surface`

Expected: compile failure, first line ``error[E0425]: cannot find type `TextureFormat` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/surface.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
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
        let config: SurfaceConfiguration = SurfaceConfiguration {
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

        self.inner.configure(device, &config);

        Ok(ConfiguredSurface {
            inner: self.inner,
            config,
        })
    }
}

/// Keeps the `SurfaceConfiguration` so a resize or a reconfigure need not derive it again.
pub struct ConfiguredSurface {
    inner: Surface<'static>,
    config: SurfaceConfiguration,
}

impl ConfiguredSurface {
    pub fn inner(&self) -> &Surface<'static> {
        &self.inner
    }

    pub fn format(&self) -> TextureFormat {
        self.config.format
    }

    pub fn size(&self) -> PhysicalSize {
        PhysicalSize {
            width_pixels: self.config.width,
            height_pixels: self.config.height,
        }
    }

    pub fn resize(&mut self, device: &Device, size: PhysicalSize) {
        let configured_size: PhysicalSize = size.at_least_one_pixel();
        self.config.width = configured_size.width_pixels;
        self.config.height = configured_size.height_pixels;

        self.inner.configure(device, &self.config);
    }

    pub fn reconfigure(&self, device: &Device) {
        self.inner.configure(device, &self.config);
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
    use wgpu::{Instance, Surface, SurfaceTarget};

    use super::WgpuSurface;
    use crate::error::AppError;

    impl WgpuSurface {
        pub fn from_canvas(instance: &Instance, canvas: HtmlCanvasElement) -> Result<WgpuSurface, AppError> {
            let created: Result<Surface<'static>, wgpu::CreateSurfaceError> =
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
    use wgpu::{Instance, Surface, SurfaceTargetUnsafe};

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
            let created: Result<Surface<'static>, wgpu::CreateSurfaceError> =
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::surface`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 494 filtered out`.

- [ ] **Step 5: Check the wasm32 build**

Run: `cargo check -p shared --target wasm32-unknown-unknown --features render`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 6: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/surface.rs
git commit -m "surface"
```

### Task 10: Game renderer

**Files:**
- Create: `shared/src/render/game_renderer.rs`
- Modify: `shared/src/render/mod.rs`
- Test: inline `mod tests` in `shared/src/render/game_renderer.rs`

`GameRenderer` creates the instance (`Instance::default()`, or GL only for `RendererBackendKind::ForceGl`, the web's `?renderer=webgl2`), the surface, a compatible adapter and a device with the WebGL2 limits, configures the surface and builds the pipelines. `upload_tick_layer` writes the tick layer's instances, which stay on the GPU until the next upload; `draw_frame` writes the frame layer and the camera uniform, clears to the backdrop colour and draws tick shapes, tick cells, projectiles and crosshair in that order. A lost surface is an error (the platform creates the renderer again); an outdated one is reconfigured and the frame skipped. Only the clear colour conversion is testable without a GPU; `./scripts/test/check-wasm.sh` checks the wasm32 build.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/render/mod.rs` with:

```rust
pub mod camera;
pub mod cell_pipeline;
pub mod game_renderer;
pub mod instance_buffer;
pub mod render_color;
pub mod scene;
pub mod scene_pipelines;
pub mod shape_pipeline;
pub mod surface;

pub use camera::*;
pub use cell_pipeline::*;
pub use game_renderer::*;
pub use instance_buffer::*;
pub use render_color::*;
pub use scene::*;
pub use scene_pipelines::*;
pub use shape_pipeline::*;
pub use surface::*;
```

Create `shared/src/render/game_renderer.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --features render --lib render::game_renderer`

Expected: compile failure, first line ``error[E0433]: cannot find type `RenderColor` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/render/game_renderer.rs`, replace everything above `#[cfg(test)]`, which is nothing yet, with the following, followed by one blank line:

```rust
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
            adapter.request_device(&scene_pipelines::get_device_descriptor()).await;
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
        let Some(surface_texture) = self.acquire_surface_texture()? else {
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
                self.surface.reconfigure(&self.device);

                Ok(Some(surface_texture))
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

        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

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
            binding: 0,
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
    render_pass.set_vertex_buffer(0, written_slice);
    render_pass.draw(0..scene_pipelines::QUAD_VERTEX_COUNT, 0..instance_buffer.count());
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --features render --lib render::game_renderer`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 496 filtered out`.

- [ ] **Step 5: Check the wasm32 builds**

Run: `./scripts/test/check-wasm.sh`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 6: Commit**

```sh
git status
git add shared/src/render/mod.rs shared/src/render/game_renderer.rs
git commit -m "game renderer"
```

### Task 11: Architecture docs for the renderer

**Files:**
- Modify: `docs/architecture/client-web.md`
- Modify: `docs/architecture/overview.md`
- Modify: `docs/architecture/testing.md`

The architecture keeps describing the code: the crosshair argument and the two layers of `RenderScene`, the renderer's construction and ownership, the shared WGSL prelude, the `params` encoding, the projectile skins, the surface format, the camera, the module tree and the renderer unit tests. Documentation only; the verification is a search for the new text.

- [ ] **Step 1: Edit the renderer section**

In `docs/architecture/client-web.md`, replace:

```text
## Renderer
- Module: `shared/src/render`, feature `render`.
- `GameRenderer` owns instance, adapter, device, queue, surface, pipelines; `!Send`.
- Surface first, then `request_adapter` with `compatible_surface: Some(&surface)` (required for WebGL2; eafora's order breaks its fallback).
- `Instance::default()` (WebGPU, else WebGL2); `?renderer=webgl2` forces `Backends::GL`; `Limits::downlevel_webgl2_defaults()`, no features, Fifo present mode.
- `surface.rs`: shared `WgpuSurface` at file level; `#[cfg(target_arch = "wasm32")] mod wasm` (`from_canvas`); `#[cfg(not(target_arch = "wasm32"))] mod native` (`from_window_handle`, for iOS later).
- Pipelines:
  - `cell_pipeline.rs` + `cell.wgsl`: one instanced draw for all cells, spores (0.8 scale) and shots; quad corners from `vertex_index`; per-instance vertex buffer (`VertexStepMode::Instance`): `center: [f32; 2]`, `color: unorm8x4`, `params: u32` (skin, scale); 16 B stride.
    - `cell.wgsl` decodes `params` and `color` only through accessor functions defined once, beside the matching Rust instance struct's field order (`fn instance_skin_kind(params: u32) -> u32`, `fn instance_scale(params: u32) -> f32`, `fn instance_color(color: vec4<f32>) -> vec4<f32>`); no call site masks, shifts or indexes channels.
  - `shape_pipeline.rs` + `shape.wgsl`: instanced rectangles and ellipses, filled or stroked: world backdrop, shadow, body, border, toxin and neutralize rings and fills, secretions, crosshair bars.
- Skins in the fragment shader: Grid (fill plus rgb(40,40,40) 0.25 px stroke), Circles (disc radius 3), Ghost (1 px outline, no fill), None (fill plus 1 px stroke in own colour).
- Camera: `world_to_view` centred on the local crosshair, `view_to_clip` orthographic with device pixel ratio; uniform `world_to_clip` (names per `shading.md`).
- Draw order: backdrop (70,70,70), shadow (backdrop minus 20, offset (+7,+6) square or (+5,+4) circle), world body (black), border (210,210,210) 1 px, toxin rings (255,111,92) weight 3, secretions (owner colour), neutralize rings (0,179,12) weight 3 for every player, organisms, spores and shots, crosshair (white plus, ±4 px).
- Cell instance buffer rebuilt only when a tick is applied; per frame only the camera uniform and interpolated projectile instances change.
- `render/scene.rs`: `RenderScene::from_state(&GameState, &InterpolationFrame, camera)`; GPU-free, unit-testable.
- WebGL2 constraints observed: no storage buffers, stride at most 255, uniform blocks at most 16 KiB.

```

with:

```text
## Renderer
- Module: `shared/src/render`, feature `render`.
- `GameRenderer` owns adapter, device, queue, surface, pipelines, the camera uniform and the instance buffers; `!Send`; built by `from_canvas` (wasm32) or `unsafe from_window_handle` (other targets) from a `RendererBackendKind` (`Default`, `ForceGl`) and a `PhysicalSize`. The `Instance` is not kept: the surface and the device hold what they need.
- Surface first, then `request_adapter` with `compatible_surface: Some(&surface)` (required for WebGL2; eafora's order breaks its fallback).
- `Instance::default()` (WebGPU, else WebGL2); `?renderer=webgl2` forces `Backends::GL` (`RendererBackendKind::ForceGl`); `Limits::downlevel_webgl2_defaults()`, no features, Fifo present mode.
- Surface format: the first without sRGB encoding, so colours and blending stay in sRGB values as the original's canvas kept them; the first format otherwise.
- `surface.rs`: shared `WgpuSurface` (unconfigured) and `ConfiguredSurface` at file level; `#[cfg(target_arch = "wasm32")] mod wasm` (`from_canvas`); `#[cfg(not(target_arch = "wasm32"))] mod native` (`from_window_handle` with a UIKit view, for iOS later).
- Pipelines (`scene_pipelines.rs`: `ScenePipelines`, one camera bind group, both built inside error scopes):
  - Every instance is a triangle strip of four vertices, corners from `vertex_index`; premultiplied alpha blending; signed-distance coverage per physical pixel for antialiasing.
  - `scene_common.wgsl`, prepended to both shaders: the camera uniform, quad corners, rectangle and ellipse distances, coverage, stroke-over-fill compositing, colour accessors.
  - `cell_pipeline.rs` + `cell.wgsl`: instanced draws for cells, spores and shots (shots at 0.8 scale, faithful); per-instance vertex buffer (`VertexStepMode::Instance`): `center: [f32; 2]`, `color: unorm8x4`, `params: u32` (skin code in the low byte, scale in thousandths of a cell in the high half); 16 B stride.
    - `cell.wgsl` decodes `params` and `color` only through accessor functions defined once, beside the matching Rust instance struct's field order (`fn instance_skin_kind(params: u32) -> u32`, `fn instance_scale(params: u32) -> f32`, `fn instance_color(color: vec4<f32>) -> vec4<f32>`); no call site masks, shifts or indexes channels.
  - `shape_pipeline.rs` + `shape.wgsl`: instanced rectangles and ellipses, filled, stroked or both (stroke centred on the edge, over the fill): shadow, world body and border, toxin and neutralize rings and fills, secretions, crosshair bars; per instance `center`, `half_extent`, fill and stroke colours (`unorm8x4`), stroke width, shape kind; 32 B stride.
- Skins in the fragment shader: Grid (fill plus rgb(40,40,40) 0.25 px stroke), Circles (disc radius 3), Ghost (1 px outline, no fill), None (fill plus 1 px stroke in own colour); spores and shots of Circles and Ghost as their cells, of Grid and None as unstroked squares (`InstanceSkinKind::Solid`, faithful).
- Colours (`render_color.rs`): the original's palette for its black world; scene colours below.
- Camera (`camera.rs`): `Camera::centred_on(crosshair, ViewportSize)` for an online game, `Camera::for_local_game` (world origin at the 25 px margin) for the title scene and the tutorial; `world_to_view` translates the view origin, `view_to_clip` is orthographic over the view size in CSS pixels (physical size over device pixel ratio); uniform `world_to_clip` plus `physical_pixels_per_world_pixel` (names per `shading.md`); `get_view_point` for name labels.
- Draw order: backdrop (70,70,70, the clear colour), shadow (backdrop minus 20, offset (+7,+6) square or (+5,+4) circle), world body (black), border (210,210,210) 1 px, toxin rings (255,111,92) weight 3, secretions (owner colour; a 2 px ring for Ghost), neutralize rings (0,179,12) weight 3 for every player, fields filled rgb(100,100,100), organisms, spores and shots, crosshair (white plus, ±4 px, when the scene is given one).
- `TickLayer` (background shapes, organism cells) uploaded only when a tick is applied (`GameRenderer::upload_tick_layer`); per frame (`draw_frame`) only the camera uniform and the `FrameLayer` (interpolated projectiles, crosshair bars) change.
- `render/scene.rs`: `RenderScene::from_state(&GameState, &InterpolationFrame, Camera, crosshair: Option<InterpolatedPoint>) -> RenderScene { tick_layer, frame_layer, camera }`; `refresh_frame` for a frame which applied no tick; GPU-free, unit-testable.
- WebGL2 constraints observed: no storage buffers, stride at most 255, uniform blocks at most 16 KiB.

```

- [ ] **Step 2: Edit the module tree**

In `docs/architecture/overview.md`, replace:

```text
render/           (feature render)       client-web.md#renderer
```

with:

```text
render/           (feature render)       client-web.md#renderer
                  render_color.rs        RenderColor, organism palette, scene colours
                  camera.rs              PhysicalSize, ViewportSize, AxisAlignedTransform, Camera, CameraUniform
                  cell_pipeline.rs       CellInstance, InstanceSkinKind, CellScale, cell pipeline (cell.wgsl)
                  shape_pipeline.rs      ShapeInstance, ShapeGeometry, ShapePaint, ShapeKind, shape pipeline (shape.wgsl)
                  scene.rs               RenderScene, TickLayer, FrameLayer (GPU-free)
                  scene_pipelines.rs     ScenePipelines, device descriptor, shared pipeline setup (scene_common.wgsl)
                  instance_buffer.rs     InstanceBuffer
                  surface.rs             WgpuSurface, ConfiguredSurface, WindowHandle (not wasm32)
                  game_renderer.rs       GameRenderer, RendererBackendKind
```

- [ ] **Step 3: Edit the unit test list**

In `docs/architecture/testing.md`, replace:

```text
- `play/message_text.rs`: every message per mode, phase and own state, singular and plural waiting texts.
```

with:

```text
- `play/message_text.rs`: every message per mode, phase and own state, singular and plural waiting texts.
- `render/scene.rs`: shadow and world per shape, fields, secretions and cells in draw order, shrunk bounds, interpolated flying projectiles, no projectile while secreting, crosshair only when given, `refresh_frame` keeps the tick layer; secretion and field radii against the simulation's.
- `render/camera.rs`: centring, local-game margin, world to clip corners, uniform layout; `render/cell_pipeline.rs`: `params` packing and skin mapping.
```

- [ ] **Step 4: Verify the edits**

Run: `grep -c "scene_common.wgsl\|crosshair: Option<InterpolatedPoint>\|game_renderer.rs\|render/scene.rs\|InstanceSkinKind" docs/architecture/client-web.md docs/architecture/overview.md docs/architecture/testing.md`

Expected:

```text
docs/architecture/client-web.md:3
docs/architecture/overview.md:3
docs/architecture/testing.md:1
```

- [ ] **Step 5: Commit**

```sh
git status
git add docs/architecture/client-web.md docs/architecture/overview.md docs/architecture/testing.md
git commit -m "architecture docs for the renderer"
```

### Task 12: Phase verification

**Files:** none; verification only.

The roadmap's done condition for this phase: the `RenderScene` unit tests and the ignored pipeline-compile test pass, with the rest of the workspace still building and passing for every target.

- [ ] **Step 1: Check formatting**

Run: `cargo fmt --all -- --check`

Expected: no output.

- [ ] **Step 2: Run every test of the workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result|FAILED'`

Expected:

```text
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 496 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Every line reads `ok`, each followed in the real output by its `finished in` timing; in order, the binaries are `protocol_dump`, the `server` library, the `server` binary, `client_session`, `game_lifecycle`, `protocol_errors`, the `shared` library (built with `render`, which `web` enables), `determinism`, `forbidden_operations`, `growth_statistics`, `protocol`, `replay`, the two `web` targets and the doc tests of `server`, `shared` and `web`.

- [ ] **Step 3: Run the shader test**

Run: `./scripts/test/test-shaders.sh`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 496 filtered out`.

- [ ] **Step 4: Check the wasm32 builds**

Run: `./scripts/test/check-wasm.sh`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 5: Confirm the working tree is clean**

Run: `git status --short`

Expected: no output other than `?? .idea/`, the untracked IDE directory.

- [ ] **Step 6: List the phase's commits**

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: renderer' --format=%h)..HEAD | cut -d' ' -f2-`

Expected, oldest first (review-fix commits, if any, sit after their task):

```text
phase 7 plan
render module and colours
camera
cell instances
shape instances
scene tick layer
scene frame layer
scene pipelines and shaders
instance buffer
surface
game renderer
architecture docs for the renderer
```

## Roadmap coverage

- `shared/src/render/` behind the `render` feature:
  - surface: Task 9 (`WgpuSurface`, `ConfiguredSurface`, surface format), used by Task 10;
  - cell pipeline: Tasks 3 (instances) and 7 (pipeline, `cell.wgsl`);
  - shape pipeline: Tasks 4 (instances) and 7 (pipeline, `shape.wgsl`);
  - WGSL: Task 7 (`scene_common.wgsl`, `cell.wgsl`, `shape.wgsl`);
  - scene construction: Tasks 5 (`TickLayer`) and 6 (`FrameLayer`, `RenderScene`), with the colours of Task 1;
  - camera: Task 2;
  - renderer: Tasks 8 (`InstanceBuffer`) and 10 (`GameRenderer`).
- Done condition:
  - `RenderScene` unit tests: Tasks 5 and 6;
  - the ignored pipeline-compile test: Task 7, through `scripts/test/test-shaders.sh`;
  - all checked together in Task 12.
- Left to later phases: choosing the camera and the crosshair per scene, creating the renderer from the canvas, resizing, uploading the tick layer after each applied tick and drawing every animation frame (the phase 8 driver); name label positions from `Camera::get_view_point` (phase 8); the WebGPU and `?renderer=webgl2` checks in Chrome and Safari at device pixel ratios 1 and 2 (manual verification).

## PR description

**Problem:** bacter-rs could simulate and follow a game but had no way to draw one.

**Solution:** `shared/src/render/` behind the `render` feature: `RenderScene::from_state` builds GPU-free instance lists from a `GameState`, a tick layer (world, fields, secretions, cells) and a frame layer (interpolated spores and shots, crosshair); `GameRenderer` creates the surface before a compatible adapter (WebGPU, WebGL2 fallback, WebGL2 limits everywhere), draws everything with two instanced pipelines whose WGSL antialiases cells, skins, rings and discs from signed distances, and re-uploads the tick layer only when a tick is applied. Unit tests cover the scene, camera and instance encodings; an ignored test builds both pipelines on a headless device (`scripts/test/test-shaders.sh`). Repository: `bacter-rs` only.
