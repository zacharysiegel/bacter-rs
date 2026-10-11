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
