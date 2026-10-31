// Colors, taken from the terminal version's 256-color palette so both look alike. The
// shellcade canvas takes RGB: `rgb` converts.

use shellcade_kit::prelude::Color;

/// Block colors, indexed by flavor: purple, blue, green, yellow, orange, gray. Close to
/// the colors of the graphical version.
pub(crate) const COLORS: [u8; 6] = [127, 62, 34, 184, 208, 244];
/// Rising row (row 0), still being pushed up: dark versions of the same colors.
pub(crate) const DIM_COLORS: [u8; 6] = [53, 17, 22, 58, 94, 238];
/// Dying block (about to vanish): a small white square at the center of its cell.
pub(crate) const DYING_COLOR: u8 = 255;
/// Garbage: normal (red) and gray.
pub(crate) const GARBAGE_COLOR: u8 = 124;
pub(crate) const GARBAGE_GRAY_COLOR: u8 = 242;
/// Brighter garbage, the moment it is hit (start of its shatter).
pub(crate) const GARBAGE_FLASH_COLOR: u8 = 167;
pub(crate) const GARBAGE_GRAY_FLASH_COLOR: u8 = 250;
/// Dots marking the playable area, and the ground line.
pub(crate) const DOT_COLOR: u8 = 240;
/// Cursor brackets.
pub(crate) const CURSOR_COLOR: u8 = 231;
/// Danger bars on each side of the board (final countdown): purple, then red for the last
/// second; the dark shades while fading out.
pub(crate) const DANGER_PURPLE: [u8; 2] = [129, 54];
pub(crate) const DANGER_RED: [u8; 2] = [196, 88];
/// Chain popup in the side panel.
pub(crate) const CHAIN_COLOR: u8 = 220;
/// Colors of the two words of the logo: red CRACK, purple ATTACK!.
pub(crate) const LOGO_COLORS: [u8; 2] = [124, 127];
/// Key reminder at the bottom of the side panel.
pub(crate) const HINT_COLOR: u8 = 244;
/// Duel: the round won, and lost.
pub(crate) const WIN_COLOR: u8 = 220;
pub(crate) const LOSS_COLOR: u8 = 160;

/// Saturation of a board drawn muted (the opponent's, a lost one): 1 is unchanged, 0 is
/// grays. Pure grays would merge blue and green blocks (lumas 109 and 103).
pub(crate) const MUTED: f32 = 0.3;

/// The RGB color of an xterm 256-color index (16 to 255: the 6×6×6 cube, then the
/// grays). The 16 system colors depend on the terminal theme; none is used here.
pub(crate) fn rgb(index: u8) -> Color {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        16..=231 => {
            let i = (index - 16) as usize;
            Color::rgb(LEVELS[i / 36], LEVELS[i / 6 % 6], LEVELS[i % 6])
        }
        232..=255 => Color::gray(8 + 10 * (index - 232)),
        _ => Color::unset(),
    }
}

/// `color` with its saturation scaled to `saturation` (1 = unchanged, 0 = its gray),
/// keeping its luma.
pub(crate) fn tint(color: Color, saturation: f32) -> Color {
    if !color.is_set() || saturation >= 1.0 {
        return color;
    }
    let (r, g, b) = color.rgb_vals();
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let mix = |c: u8| (luma + saturation * (c as f32 - luma)).round().clamp(0.0, 255.0) as u8;
    Color::rgb(mix(r), mix(g), mix(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_indices_turn_into_their_rgb() {
        assert_eq!(rgb(16).rgb_vals(), (0, 0, 0));
        assert_eq!(rgb(231).rgb_vals(), (255, 255, 255));
        assert_eq!(rgb(124).rgb_vals(), (175, 0, 0));
        assert_eq!(rgb(208).rgb_vals(), (255, 135, 0));
        assert_eq!(rgb(240).rgb_vals(), (88, 88, 88));
    }

    #[test]
    fn a_muted_color_keeps_its_luma() {
        let (r, g, b) = tint(rgb(34), 0.0).rgb_vals();
        assert!(r == g && g == b && (102..=104).contains(&r));
        assert_eq!(tint(rgb(34), 1.0), rgb(34));
        assert!(!tint(Color::unset(), 0.0).is_set());
    }
}
