//! `MaskedText`: what a Secret or Suspected Item shows instead of its value.
//!
//! A Secret is a solid bar, a Suspected Item the security-tint hatch. Both
//! are a fixed width, so the Mask tells nothing about the value's length
//! (`docs/design/README.md`, ADR 0003). The colour comes from CSS
//! (`.mask-secret`, `.mask-suspected`, which use the token colours); the
//! geometry comes from the constants below, checked against
//! `design/tokens.json` by a test.

use gtk::prelude::*;

/// `mask.secret-width-ch`: how many characters wide every Mask is.
pub const WIDTH_CH: i32 = 13;
/// `mask.hatch-line-px`: thickness of a hatch line.
pub const HATCH_LINE_PX: f64 = 1.6;
/// `mask.hatch-gap-px`: space between hatch lines.
pub const HATCH_GAP_PX: f64 = 4.0;
/// `mask.reveal-max-ms`: the longest a Reveal lasts, even while held.
pub const REVEAL_MAX_MS: u64 = 5_000;
/// `radius.chip`: corner radius of the Mask.
const RADIUS_PX: f64 = 4.0;

/// Which Mask to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Solid bar.
    Solid,
    /// Security-tint hatch.
    Hatch,
}

/// A Mask widget of the given shape, sized to [`WIDTH_CH`] characters of
/// the monospace Item font.
#[must_use]
pub fn masked_text(shape: Shape) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("mask");
    area.add_css_class(match shape {
        Shape::Solid => "mask-secret",
        Shape::Hatch => "mask-suspected",
    });
    area.set_valign(gtk::Align::Center);
    area.set_halign(gtk::Align::Start);
    // Screen readers hear the row's label, never anything from the Mask.
    area.set_accessible_role(gtk::AccessibleRole::Presentation);

    let (char_width, line_height) = char_size(&area);
    area.set_content_width(char_width * WIDTH_CH);
    area.set_content_height(line_height);

    area.set_draw_func(move |area, cr, width, height| {
        let colour = area.color();
        cr.set_source_rgba(
            f64::from(colour.red()),
            f64::from(colour.green()),
            f64::from(colour.blue()),
            f64::from(colour.alpha()),
        );
        rounded_rect(cr, f64::from(width), f64::from(height));
        match shape {
            Shape::Solid => {
                let _ = cr.fill();
            }
            Shape::Hatch => {
                cr.clip();
                cr.set_line_width(HATCH_LINE_PX);
                let (width, height) = (f64::from(width), f64::from(height));
                let step = HATCH_LINE_PX + HATCH_GAP_PX;
                let mut x = -height;
                while x < width {
                    cr.move_to(x, height);
                    cr.line_to(x + height, 0.0);
                    x += step;
                }
                let _ = cr.stroke();
            }
        }
    });
    area
}

/// Width of one character and the line height, in the Item font.
fn char_size(widget: &impl IsA<gtk::Widget>) -> (i32, i32) {
    let layout = widget.create_pango_layout(Some("0"));
    let mut font = gtk::pango::FontDescription::new();
    font.set_family("Adwaita Mono, monospace");
    layout.set_font_description(Some(&font));
    let (width, height) = layout.pixel_size();
    (width.max(1), height.max(1))
}

fn rounded_rect(cr: &gtk::cairo::Context, width: f64, height: f64) {
    let r = RADIUS_PX.min(height / 2.0);
    let quarter = std::f64::consts::FRAC_PI_2;
    cr.new_sub_path();
    cr.arc(width - r, r, r, -quarter, 0.0);
    cr.arc(width - r, height - r, r, 0.0, quarter);
    cr.arc(r, height - r, r, quarter, 2.0 * quarter);
    cr.arc(r, r, r, 2.0 * quarter, 3.0 * quarter);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::{HATCH_GAP_PX, HATCH_LINE_PX, RADIUS_PX, REVEAL_MAX_MS, WIDTH_CH};

    /// The geometry here must match the single source in design/tokens.json.
    #[test]
    fn geometry_matches_the_tokens() {
        let tokens: serde_json::Value =
            serde_json::from_str(include_str!("../../../design/tokens.json")).unwrap();
        let mask = &tokens["mask"];
        assert_eq!(mask["secret-width-ch"].as_i64(), Some(i64::from(WIDTH_CH)));
        assert_eq!(mask["hatch-line-px"].as_f64(), Some(HATCH_LINE_PX));
        assert_eq!(mask["hatch-gap-px"].as_f64(), Some(HATCH_GAP_PX));
        assert_eq!(mask["reveal-max-ms"].as_u64(), Some(REVEAL_MAX_MS));
        assert_eq!(tokens["radius"]["chip"].as_f64(), Some(RADIUS_PX));
    }
}
