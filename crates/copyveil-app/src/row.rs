//! `ItemRow`: one Item in the popup.
//!
//! Number for <kbd>Ctrl</kbd>+number, type glyph, content (or its Mask), a
//! meta line, and a pin mark. The row keeps a fixed height, so a Reveal
//! doesn't make the list jump. What a screen reader hears comes from
//! [`crate::text::accessible_label`], never from the content.

use crate::mask::{self, MaskedText, Shape};
use crate::ring::ExpiryRing;
use crate::text;
use copyveil_dbus::{ItemSummary, kind, sensitivity};
use gtk::glib;
use gtk::prelude::*;
use std::time::{Duration, Instant};

/// `motion.expiry-collapse`: the row closing after the Mask dissolves.
pub const EXPIRY_COLLAPSE_MS: u32 = 460;

/// What every row is built with, beyond its Item.
#[derive(Debug, Clone)]
pub struct RowContext {
    /// When the Items were listed; ages and Expiry count from here.
    pub listed_at: Instant,
    /// How long a Secret lives, for the `ExpiryRing`'s full circle.
    pub secret_expiry: Duration,
    /// How long a Suspected Item lives.
    pub suspected_expiry: Duration,
    /// The search, highlighted in Plain previews.
    pub query: String,
}

impl RowContext {
    /// Listed now, with the default Expiry times and no search.
    #[must_use]
    pub fn now() -> Self {
        Self {
            listed_at: Instant::now(),
            secret_expiry: Duration::from_secs(30),
            suspected_expiry: Duration::from_mins(15),
            query: String::new(),
        }
    }
}

/// The parts of a row that change after it's built.
pub struct ItemRow {
    /// The Item id.
    pub id: u64,
    /// The row widget, for the list.
    pub widget: gtk::ListBoxRow,
    /// Whether the content is masked (Secret or Suspected).
    pub masked: bool,
    /// "Not a secret", on Suspected Items only. The popup connects it.
    pub unflag: Option<gtk::Button>,
    /// The Mask, for masked Items.
    mask: Option<MaskedText>,
    /// Under the Mask: the value while it's revealed, empty otherwise.
    revealed: gtk::Label,
    /// Collapses the row at Expiry.
    collapser: gtk::Revealer,
    meta: gtk::Label,
    age_ms: u64,
    expires_at: Option<Instant>,
    reason: String,
    pin_slot: u32,
    listed_at: Instant,
    ring: Option<(ExpiryRing, Duration)>,
}

impl ItemRow {
    /// Builds the row for `item`, numbered `number` when it is among the first nine.
    #[must_use]
    pub fn new(item: &ItemSummary, number: Option<usize>, row: &RowContext) -> Self {
        let listed_at = row.listed_at;
        let masked = item.sensitivity != sensitivity::PLAIN;
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.add_css_class("item-row");

        let number_label = gtk::Label::new(number.map(|n| n.to_string()).as_deref());
        number_label.add_css_class("item-number");
        number_label.set_width_chars(1);
        line.append(&number_label);

        let glyph = gtk::Image::from_icon_name(glyph_for(item));
        glyph.add_css_class("item-glyph");
        line.append(&glyph);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
        body.set_hexpand(true);

        let revealed = content_label("");
        let mask = masked.then(|| MaskedText::new(shape_for(item)));
        if let Some(mask) = &mask {
            // The value sits under the Mask, so a Reveal peels the Mask off it.
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&revealed));
            overlay.add_overlay(&mask.area);
            overlay.set_measure_overlay(&mask.area, true);
            body.append(&overlay);
        } else if item.kind == kind::IMAGE {
            let label = content_label(&format!("Image · {}", item.mime));
            label.add_css_class("dim-label");
            body.append(&label);
        } else {
            let preview = content_label(&first_line(&item.preview));
            highlight(&preview, &row.query);
            body.append(&preview);
        }

        let meta = gtk::Label::new(None);
        meta.add_css_class("item-meta");
        meta.add_css_class("caption");
        meta.set_xalign(0.0);
        let meta_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        if masked {
            meta_line.append(&mask::badge(shape_for(item)));
        }
        meta_line.append(&meta);
        body.append(&meta_line);
        line.append(&body);

        let ring = (item.expires_in_ms > 0).then(|| {
            let total = if item.sensitivity == sensitivity::SECRET {
                row.secret_expiry
            } else {
                row.suspected_expiry
            };
            let ring = ExpiryRing::new(1.0);
            ring.area.set_tooltip_text(Some("Time until it’s erased"));
            line.append(&ring.area);
            (ring, total)
        });

        let unflag = (item.sensitivity == sensitivity::SUSPECTED).then(|| {
            let button = gtk::Button::with_label("Not a secret");
            button.add_css_class("flat");
            button.add_css_class("item-unflag");
            button.set_valign(gtk::Align::Center);
            button.set_tooltip_text(Some("Show and keep it as Plain (Ctrl+N)"));
            line.append(&button);
            button
        });

        if item.pin_slot > 0 {
            let pin = gtk::Image::from_icon_name("view-pin-symbolic");
            pin.add_css_class("item-pin");
            pin.set_tooltip_text(Some(&format!("Pin slot {}", item.pin_slot)));
            line.append(&pin);
        }

        let collapser = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(EXPIRY_COLLAPSE_MS)
            .reveal_child(true)
            .child(&line)
            .build();
        let widget = gtk::ListBoxRow::builder().child(&collapser).build();
        widget.update_property(&[gtk::accessible::Property::Label(&text::accessible_label(
            item,
        ))]);

        let row = Self {
            id: item.id,
            widget,
            masked,
            unflag,
            mask,
            revealed,
            collapser,
            meta,
            age_ms: item.age_ms,
            expires_at: (item.expires_in_ms > 0)
                .then(|| listed_at + Duration::from_millis(item.expires_in_ms)),
            reason: item.reason.clone(),
            pin_slot: item.pin_slot,
            listed_at,
            ring,
        };
        row.tick(listed_at);
        row
    }

    /// Updates the meta line (age, reason, time left) for `now`.
    pub fn tick(&self, now: Instant) {
        let elapsed = millis(now.saturating_duration_since(self.listed_at));
        let mut parts = vec![text::age(self.age_ms.saturating_add(elapsed))];
        if !self.reason.is_empty() {
            parts.push(self.reason.clone());
        }
        if let (Some(at), Some((ring, total))) = (self.expires_at, &self.ring) {
            let left = millis(at.saturating_duration_since(now));
            ring.set_fraction(text::expiry_fraction(left, millis(*total)));
        }
        if let Some(at) = self.expires_at {
            parts.push(format!(
                "clears in {}",
                text::time_left(millis(at.saturating_duration_since(now)).max(1))
            ));
        }
        if self.pin_slot > 0 {
            parts.push(format!("Pin {}", self.pin_slot));
        }
        self.meta.set_text(&parts.join(" · "));
    }

    /// When the Item's Expiry comes, for masked Items.
    #[must_use]
    pub fn expiry_deadline(&self) -> Option<Instant> {
        self.expires_at
    }

    /// Draws the Mask in: for a sensitive Item that just arrived.
    pub fn draw_in(&self) {
        if let Some(mask) = &self.mask {
            mask.draw_in();
        }
    }

    /// Peels the Mask off `value` until [`ItemRow::conceal`].
    pub fn reveal(&self, value: &str) {
        if let Some(mask) = &self.mask {
            self.revealed.set_text(&first_line(value));
            mask.peel();
        }
    }

    /// Drops the Mask back, then forgets the revealed value.
    pub fn conceal(&self) {
        if let Some(mask) = &self.mask
            && !self.revealed.text().is_empty()
        {
            mask.drop_back();
            let revealed = self.revealed.clone();
            glib::timeout_add_local_once(mask::motion(mask::REVEAL_PEEL_MS), move || {
                revealed.set_text("");
            });
        }
    }

    /// The Expiry: the Mask dissolves, then the row collapses, then `done`
    /// runs. No tombstone is left.
    pub fn expire(&self, done: impl Fn() + 'static) {
        let collapser = self.collapser.clone();
        let collapse = move || {
            collapser.set_reveal_child(false);
        };
        match &self.mask {
            Some(mask) => mask.dissolve(collapse),
            None => collapse(),
        }
        let total = mask::motion(mask::EXPIRY_DISSOLVE_MS + EXPIRY_COLLAPSE_MS);
        glib::timeout_add_local_once(total, done);
    }
}

fn shape_for(item: &ItemSummary) -> Shape {
    if item.sensitivity == sensitivity::SECRET {
        Shape::Solid
    } else {
        Shape::Hatch
    }
}

fn glyph_for(item: &ItemSummary) -> &'static str {
    match (item.sensitivity.as_str(), item.kind.as_str()) {
        (sensitivity::SECRET, _) => "dialog-password-symbolic",
        (sensitivity::SUSPECTED, _) => "security-medium-symbolic",
        (_, kind::IMAGE) => "image-x-generic-symbolic",
        (_, kind::RICH_TEXT) => "format-text-rich-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

/// Marks where the search matches in a Plain preview, in the Marker
/// colour (`design/tokens.json`) for the current theme.
fn highlight(label: &gtk::Label, query: &str) {
    let ranges = text::search_matches(&label.text(), query);
    if ranges.is_empty() {
        return;
    }
    let dark = adw::StyleManager::default().is_dark();
    let pick = |(light, dark_colour): crate::tokens::Themed| if dark { dark_colour } else { light };
    let channel = |value: f32| {
        // Colours are 0..=1, so this is 0..=65535.
        let scaled = (value.clamp(0.0, 1.0) * 65535.0).round();
        format!("{scaled:.0}").parse::<u16>().unwrap_or(u16::MAX)
    };
    let attributes = gtk::pango::AttrList::new();
    for range in ranges {
        let (r, g, b) = pick(crate::tokens::MARKER);
        let mut background =
            gtk::pango::AttrColor::new_background(channel(r), channel(g), channel(b));
        let (r, g, b) = pick(crate::tokens::ON_MARKER);
        let mut foreground =
            gtk::pango::AttrColor::new_foreground(channel(r), channel(g), channel(b));
        let (start, end) = (
            u32::try_from(range.start).unwrap_or(u32::MAX),
            u32::try_from(range.end).unwrap_or(u32::MAX),
        );
        background.set_start_index(start);
        background.set_end_index(end);
        foreground.set_start_index(start);
        foreground.set_end_index(end);
        attributes.insert(background);
        attributes.insert(foreground);
    }
    label.set_attributes(Some(&attributes));
}

fn content_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("item-content");
    label.add_css_class("monospace");
    label.set_xalign(0.0);
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label
}

/// The first line of a value, with a mark when there are more.
fn first_line(value: &str) -> String {
    let trimmed = value.trim_start();
    match trimmed.split_once('\n') {
        Some((first, _)) => format!("{first} …"),
        None => trimmed.to_owned(),
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::first_line;

    #[test]
    fn multi_line_values_show_their_first_line() {
        assert_eq!(first_line("git status"), "git status");
        assert_eq!(first_line("  cd /tmp\nls"), "cd /tmp …");
        assert_eq!(first_line(""), "");
    }
}
