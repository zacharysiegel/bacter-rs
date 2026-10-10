// Steps of the original's pixel pen: L left, U up, R right, D down.
const EXTEND_PEN_PATH: &str = concat!(
    "DDLLLLLLLLDDDDLUUUUURULURULURULURULDLDDDDDDDDDDULUUUUUUUUDLDDDDDDULUUUUDLDDRRRRRRRUUURDDDRUUURDD",
    "DRUUURDDDRUUURDRUDDDDRUUUURDDDDRUUUURDDDDRUUUURDDDDRDDDDRUUUUULURULURULURULURDRDDDDDDDDDDURUUUUU",
    "UUUDRDDDDDDURUUUUDRDD",
);
const COMPRESS_PEN_PATH: &str = concat!(
    "LLLLLLLLLLLLLUUDDDDRUUUURDDDDRUUUURDDDDRUUUURDDDDRDDDDRUUUUULURULURULURULURDRDDDDDDDDDDURUUUUUUU",
    "UDRDDDDDDURUUUUDRDDRDUUUURUDDDDDDRDUUUUUUUURUDDDDDDDDDDRDRUULURULURULURULURULURULRDDDDRDDDDRUUUU",
    "RDDDDRUUUURDDDDRUUUURDDDDRUUUU",
);
const IMMORTALITY_PEN_PATH: &str = concat!(
    "DDLUDDLUDDLUDDLULDDRLLDLLURRULLLDLURULLDULURLLURLLURULURULRRULRRULRRRULLRURRUDDRUURDDRURDDLRRUDR",
    "DLRRDLRDRURDURULRURDUURURDDLRURUURDDRUUDRRDLDRRUDDRUDRDLRRDLDRDLDRLDLUDDLUDDLUDDLUUDLDDLUULDDULU",
    "ULDDULUULDUULDUU",
);
const FREEZE_PEN_PATH: &str = concat!(
    "RRLLLLLULRRRRRLULLLLLULRRRRRLULLLLLULLLRRRRRRRLULLLLLURRRRULLLRURRRRRULLLRRRURDRURDRUDDRURDRUDDR",
    "URDRDLRRDLDRRDLDRDRDLDRDLDRLDLDRDLLLLLLLLULRRRRRLULLLLLULRRRRRLULLLLLRDRDRDRDRDRRRRRRDLLLLLRRRRD",
    "LLLDRRLLLLLDRRRLLLDLULDLULDUULDLULDUULDLULURLLURULLURULURLLURULURULRURULUR",
);
const NEUTRALIZE_PEN_PATH: &str = concat!(
    "RRUUUUURRRRRRUUUURUULULRDRDRDRDRDRDULULULLLDLRRRDLLLLDRRRLLLLLUDLDLRRRRRRLDLLLLLLDLRRRRRRRRLDLLL",
    "LLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLRDRRRRRLDLDL",
    "ULLDLDLDDLU",
);
const TOXIN_PEN_PATH: &str = concat!(
    "LLLLLLDDDDDDDDDDDLDLLLULRRRRRULLLLLLRRRRRRRRRRRRRRRRRRRRDLLLLLLLLDRRRRRRRRLDLLLLLLUUUURRRRRRRLLL",
    "LLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRRRULLLLLLLLLLLLLLLLLLLRURRRRRRRRRRRRRRRRRRRURLLLLLLLLLLLLLLLL",
    "LLLLRURRRRRRRRRRRRRRRRRRRRURRRLLLLLLLLLLLLLLLLLLLLLLRRURRRRRRRRRRRRRRRRRRRRRRURLLLLLLLLLLLLLLLLL",
    "LLLLLURRRRRRRRRRRRRRRRRRRRRRULLLLLLLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRRRRRRLULLLLLLLLLLLLLLLLLLLL",
    "LULRRRRRRRRRRRRRRRRRRRRRRLLULLLLLLLLLLLLLLLLLLLLLULRRRRRRRRRRRRRRRLULLLLLLLLLLLLLLLURRRRRRRRRRRR",
    "RRRRURLLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRULLLLLLLLDLLLLULLLLRRRRDRRRRUURRRRRRRRLLULLLLL",
);
const SPORE_PEN_PATH: &str = concat!(
    "ULUULULUUUURURRRRDRRRLLLLLLLLLLDLRRRLLDLLDRLLDRDLDLDRDLDRDLRDRDLDRRDLRRDLRRRDLLRRRDRURDRURDURRRU",
    "LLRRRULRRULRRULURURULURULURLULURULLURLLURLLLULLLDDDDUUUURRRDDDDLDLDDRRURURRRRDDDLLLLRRRRDDDLLLLU",
    "LULLDDRDRDDDDLLLUUUUDDDDLLLUUUURURUULLDLDLLLLUUURRRRLLLLUUURRRRDRDRRDDRRUUL",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityIconKind {
    Extend,
    Compress,
    Immortality,
    Freeze,
    Neutralize,
    Toxin,
    Spore,
    Secrete,
}

impl AbilityIconKind {
    fn pen_path(self) -> &'static str {
        match self {
            AbilityIconKind::Extend => EXTEND_PEN_PATH,
            AbilityIconKind::Compress => COMPRESS_PEN_PATH,
            AbilityIconKind::Immortality => IMMORTALITY_PEN_PATH,
            AbilityIconKind::Freeze => FREEZE_PEN_PATH,
            AbilityIconKind::Neutralize => NEUTRALIZE_PEN_PATH,
            AbilityIconKind::Toxin => TOXIN_PEN_PATH,
            AbilityIconKind::Spore | AbilityIconKind::Secrete => SPORE_PEN_PATH,
        }
    }
}

/// One pixel of an icon, offset from the centre of its ability slot in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconPixel {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenStepKind {
    Left,
    Up,
    Right,
    Down,
}

impl PenStepKind {
    fn from_letter(letter: char) -> Option<PenStepKind> {
        match letter {
            'L' => Some(PenStepKind::Left),
            'U' => Some(PenStepKind::Up),
            'R' => Some(PenStepKind::Right),
            'D' => Some(PenStepKind::Down),
            _ => None,
        }
    }

    fn offset(self) -> (i32, i32) {
        match self {
            PenStepKind::Left => (-1, 0),
            PenStepKind::Up => (0, -1),
            PenStepKind::Right => (1, 0),
            PenStepKind::Down => (0, 1),
        }
    }
}

/// In drawing order, repeats included; the pen starts one pixel up and left of the slot centre (faithful).
pub fn get_icon_pixels(icon: AbilityIconKind) -> Vec<IconPixel> {
    let mut pen: IconPixel = IconPixel { x: -1, y: -1 };
    let mut icon_pixels: Vec<IconPixel> = vec![pen];

    for pen_step in icon.pen_path().chars().filter_map(PenStepKind::from_letter) {
        let (offset_x, offset_y): (i32, i32) = pen_step.offset();
        pen = IconPixel {
            x: pen.x + offset_x,
            y: pen.y + offset_y,
        };
        icon_pixels.push(pen);
    }

    icon_pixels
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const ICONS: [AbilityIconKind; 8] = [
        AbilityIconKind::Extend,
        AbilityIconKind::Compress,
        AbilityIconKind::Immortality,
        AbilityIconKind::Freeze,
        AbilityIconKind::Neutralize,
        AbilityIconKind::Toxin,
        AbilityIconKind::Spore,
        AbilityIconKind::Secrete,
    ];

    /// Distinct pixels and bounding box (minimum x, maximum x, minimum y, maximum y) of the original's drawings.
    fn get_original_icon_shape(icon: AbilityIconKind) -> (usize, (i32, i32, i32, i32)) {
        match icon {
            AbilityIconKind::Extend => (197, (-15, 13, -7, 5)),
            AbilityIconKind::Compress => (194, (-14, 13, -7, 5)),
            AbilityIconKind::Immortality => (173, (-15, 13, -8, 6)),
            AbilityIconKind::Freeze => (220, (-12, 10, -12, 10)),
            AbilityIconKind::Neutralize => (156, (-11, 12, -13, 11)),
            AbilityIconKind::Toxin => (498, (-13, 15, -14, 12)),
            AbilityIconKind::Spore | AbilityIconKind::Secrete => (189, (-10, 8, -10, 8)),
        }
    }

    #[test]
    fn pen_paths_hold_only_step_letters() {
        for icon in ICONS {
            assert!(icon.pen_path().chars().all(|letter| PenStepKind::from_letter(letter).is_some()));
        }
    }

    #[test]
    fn get_icon_pixels_draws_one_pixel_per_step_from_up_left_of_the_centre() {
        let icon_pixels: Vec<IconPixel> = get_icon_pixels(AbilityIconKind::Freeze);

        assert_eq!(icon_pixels.len(), 267);
        assert_eq!(icon_pixels[0], IconPixel { x: -1, y: -1 });
        assert_eq!(icon_pixels[1], IconPixel { x: 0, y: -1 });
    }

    #[test]
    fn get_icon_pixels_reproduces_the_original_drawings() {
        for icon in ICONS {
            let icon_pixels: Vec<IconPixel> = get_icon_pixels(icon);
            let distinct_pixels: BTreeSet<(i32, i32)> =
                icon_pixels.iter().map(|icon_pixel| (icon_pixel.x, icon_pixel.y)).collect();
            let bounding_box: (i32, i32, i32, i32) = (
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).max().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).max().unwrap(),
            );

            assert_eq!((distinct_pixels.len(), bounding_box), get_original_icon_shape(icon));
        }
    }
}
