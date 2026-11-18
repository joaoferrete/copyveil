//! `ExpiryRing`: a small ring that empties until an Item's Expiry.
//!
//! It draws in the CSS colour of `.expiry-ring` (Slate, from the tokens): a
//! faint full circle, and over it the arc of time left, starting at the top.

use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// Ring diameter, in pixels.
const SIZE_PX: i32 = 16;
const LINE_PX: f64 = 2.5;

/// A ring and the fraction of time it shows as left.
#[derive(Clone)]
pub struct ExpiryRing {
    /// The widget.
    pub area: gtk::DrawingArea,
    fraction: Rc<Cell<f64>>,
}

impl ExpiryRing {
    /// A ring showing `fraction` (1 just copied, 0 due) of the time left.
    #[must_use]
    pub fn new(fraction: f64) -> Self {
        let area = gtk::DrawingArea::new();
        area.add_css_class("expiry-ring");
        area.set_content_width(SIZE_PX);
        area.set_content_height(SIZE_PX);
        area.set_valign(gtk::Align::Center);
        area.set_accessible_role(gtk::AccessibleRole::Presentation);
        let fraction = Rc::new(Cell::new(fraction));
        let shown = Rc::clone(&fraction);
        area.set_draw_func(move |area, cr, width, height| {
            let colour = area.color();
            let (red, green, blue) = (
                f64::from(colour.red()),
                f64::from(colour.green()),
                f64::from(colour.blue()),
            );
            let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
            let radius = cx.min(cy) - LINE_PX / 2.0;
            cr.set_line_width(LINE_PX);
            cr.set_source_rgba(red, green, blue, 0.2);
            cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
            let left = shown.get();
            if left > 0.0 {
                let start = -std::f64::consts::FRAC_PI_2;
                cr.set_source_rgba(red, green, blue, 1.0);
                cr.arc(cx, cy, radius, start, start + left * std::f64::consts::TAU);
                let _ = cr.stroke();
            }
        });
        Self { area, fraction }
    }

    /// Shows a new fraction of time left.
    pub fn set_fraction(&self, fraction: f64) {
        if (self.fraction.get() - fraction).abs() > f64::EPSILON {
            self.fraction.set(fraction);
            self.area.queue_draw();
        }
    }
}
