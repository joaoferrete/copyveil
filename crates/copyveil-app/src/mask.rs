//! `MaskedText`: what a Secret or Suspected Item shows instead of its value.
//!
//! A Secret is a solid bar, a Suspected Item the security-tint hatch. Both
//! are a fixed width, so the Mask tells nothing about the value's length
//! (`docs/design/README.md`, ADR 0003). The colour comes from CSS
//! (`.mask-secret`, `.mask-suspected`, which use the token colours); the
//! geometry comes from the constants below, checked against
//! `design/tokens.json` by a test, as are the motion timings.

use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

/// `motion.veil-draw`: the Mask drawing in when a sensitive Item arrives.
const VEIL_DRAW_MS: u32 = 280;
/// `motion.reveal-peel`: the Mask peeling off, and back, on a Reveal.
pub const REVEAL_PEEL_MS: u32 = 170;
/// `motion.expiry-dissolve`: the Mask dissolving at Expiry.
pub const EXPIRY_DISSOLVE_MS: u32 = 300;

/// How long to wait for an animation of `ms`: zero when the system asks
/// for reduced motion, since libadwaita then skips the animation itself.
#[must_use]
pub fn motion(ms: u32) -> std::time::Duration {
    let animated =
        gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
    std::time::Duration::from_millis(if animated { u64::from(ms) } else { 0 })
}

/// `MaskedText`: a Mask that can draw in, peel off and dissolve.
///
/// It draws only the span `[start, end]` of its width (fractions from 0 to
/// 1): the veil draw grows `end` from 0, a Reveal moves `start` to 1 like
/// tape peeled off to the right, and dropping it back moves `start` to 0.
/// Animations are libadwaita's, so they turn into instant changes when the
/// system asks for reduced motion.
#[derive(Clone)]
pub struct MaskedText {
    /// The widget.
    pub area: gtk::DrawingArea,
    span: Rc<Cell<(f64, f64)>>,
    animation: Rc<RefCell<Option<adw::TimedAnimation>>>,
}

impl MaskedText {
    /// A Mask of `shape`, [`WIDTH_CH`] characters of the Item font wide.
    #[must_use]
    pub fn new(shape: Shape) -> Self {
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

        let span = Rc::new(Cell::new((0.0, 1.0)));
        let shown = Rc::clone(&span);
        area.set_draw_func(move |area, cr, width, height| {
            let (start, end) = shown.get();
            if end <= start {
                return;
            }
            let w = f64::from(width);
            cr.rectangle(start * w, 0.0, (end - start) * w, f64::from(height));
            cr.clip();
            paint(area, cr, width, height, shape, RADIUS_PX);
        });
        Self {
            area,
            span,
            animation: Rc::new(RefCell::new(None)),
        }
    }

    /// The veil draw: the Mask grows from the left. The value is never shown first.
    pub fn draw_in(&self) {
        self.span.set((0.0, 0.0));
        self.animate(
            0.0,
            1.0,
            VEIL_DRAW_MS,
            adw::Easing::EaseOutQuart,
            |span, v| (span.0, v),
        );
    }

    /// The Reveal: the Mask peels off to the right.
    pub fn peel(&self) {
        let from = self.span.get().0;
        self.animate(
            from,
            1.0,
            REVEAL_PEEL_MS,
            adw::Easing::EaseOutCubic,
            |span, v| (v, span.1),
        );
    }

    /// The Reveal ends: the Mask drops back.
    pub fn drop_back(&self) {
        let from = self.span.get().0;
        self.animate(
            from,
            0.0,
            REVEAL_PEEL_MS,
            adw::Easing::EaseOutCubic,
            |span, v| (v, span.1),
        );
    }

    /// The Expiry: the Mask dissolves, then `done` runs.
    pub fn dissolve(&self, done: impl Fn() + 'static) {
        let area = self.area.clone();
        let target = adw::CallbackAnimationTarget::new(move |value| area.set_opacity(value));
        let animation = adw::TimedAnimation::new(&self.area, 1.0, 0.0, EXPIRY_DISSOLVE_MS, target);
        animation.set_easing(adw::Easing::EaseInCubic);
        animation.connect_done(move |_| done());
        animation.play();
        self.animation.replace(Some(animation));
    }

    fn animate(
        &self,
        from: f64,
        to: f64,
        duration_ms: u32,
        easing: adw::Easing,
        apply: impl Fn((f64, f64), f64) -> (f64, f64) + 'static,
    ) {
        let (span, area) = (Rc::clone(&self.span), self.area.clone());
        let target = adw::CallbackAnimationTarget::new(move |value| {
            span.set(apply(span.get(), value));
            area.queue_draw();
        });
        let animation = adw::TimedAnimation::new(&self.area, from, to, duration_ms, target);
        animation.set_easing(easing);
        animation.play();
        // Kept so it isn't dropped mid-play; a new one replaces it.
        self.animation.replace(Some(animation));
    }
}

/// `SensitivityBadge`: a small swatch in the Mask's shape plus its name, so
/// the Sensitivity reads by shape and word, never by colour alone. Plain
/// Items have no badge.
#[must_use]
pub fn badge(shape: Shape) -> gtk::Box {
    let swatch = gtk::DrawingArea::new();
    swatch.add_css_class(match shape {
        Shape::Solid => "mask-secret",
        Shape::Hatch => "mask-suspected",
    });
    swatch.set_content_width(BADGE_PX);
    swatch.set_content_height(BADGE_PX);
    swatch.set_valign(gtk::Align::Center);
    swatch.set_accessible_role(gtk::AccessibleRole::Presentation);
    swatch.set_draw_func(move |area, cr, width, height| {
        paint(area, cr, width, height, shape, BADGE_RADIUS_PX);
    });

    let label = gtk::Label::new(Some(match shape {
        Shape::Solid => "Secret",
        Shape::Hatch => "Suspected",
    }));
    label.add_css_class("caption-heading");
    let badge = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    badge.add_css_class("sensitivity-badge");
    badge.append(&swatch);
    badge.append(&label);
    badge
}

/// `SensitivityBadge` swatch size and corner radius: small enough to stay a
/// square, like the Mask it stands for.
const BADGE_PX: i32 = 9;
const BADGE_RADIUS_PX: f64 = 1.5;

/// Width of one character and the line height, in the Item font.
fn char_size(widget: &impl IsA<gtk::Widget>) -> (i32, i32) {
    let layout = widget.create_pango_layout(Some("0"));
    let mut font = gtk::pango::FontDescription::new();
    font.set_family("Adwaita Mono, monospace");
    layout.set_font_description(Some(&font));
    let (width, height) = layout.pixel_size();
    (width.max(1), height.max(1))
}

/// Paints a Mask shape in the widget's CSS colour.
fn paint(
    area: &gtk::DrawingArea,
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    shape: Shape,
    radius: f64,
) {
    let colour = area.color();
    cr.set_source_rgba(
        f64::from(colour.red()),
        f64::from(colour.green()),
        f64::from(colour.blue()),
        f64::from(colour.alpha()),
    );
    rounded_rect(cr, f64::from(width), f64::from(height), radius);
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
}

fn rounded_rect(cr: &gtk::cairo::Context, width: f64, height: f64, radius: f64) {
    let r = radius.min(height / 2.0);
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
    use super::{
        EXPIRY_DISSOLVE_MS, HATCH_GAP_PX, HATCH_LINE_PX, RADIUS_PX, REVEAL_MAX_MS, REVEAL_PEEL_MS,
        VEIL_DRAW_MS, WIDTH_CH,
    };

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
        let motion = &tokens["motion"];
        assert_eq!(
            motion["veil-draw"]["ms"].as_u64(),
            Some(u64::from(VEIL_DRAW_MS))
        );
        assert_eq!(
            motion["reveal-peel"]["ms"].as_u64(),
            Some(u64::from(REVEAL_PEEL_MS))
        );
        assert_eq!(
            motion["expiry-dissolve"]["ms"].as_u64(),
            Some(u64::from(EXPIRY_DISSOLVE_MS))
        );
        assert_eq!(
            motion["expiry-collapse"]["ms"].as_u64(),
            Some(u64::from(crate::row::EXPIRY_COLLAPSE_MS))
        );
        assert_eq!(
            motion["carbon-copy"]["ms"].as_u64(),
            Some(u64::from(crate::window::CARBON_COPY_MS))
        );
    }
}
