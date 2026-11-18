//! `ItemRow`: one Item in the popup.
//!
//! Number for <kbd>Ctrl</kbd>+number, type glyph, content (or its Mask), a
//! meta line, and a pin mark. The row keeps a fixed height, so a Reveal
//! doesn't make the list jump. What a screen reader hears comes from
//! [`crate::text::accessible_label`], never from the content.

use crate::mask::{self, Shape};
use crate::text;
use copyveil_dbus::{ItemSummary, kind, sensitivity};
use gtk::prelude::*;
use std::time::{Duration, Instant};

/// The parts of a row that change after it's built.
pub struct ItemRow {
    /// The Item id.
    pub id: u64,
    /// The row widget, for the list.
    pub widget: gtk::ListBoxRow,
    /// Whether the content is masked (Secret or Suspected).
    pub masked: bool,
    /// Shows `mask` or `revealed`, for a Reveal.
    content: gtk::Stack,
    revealed: gtk::Label,
    meta: gtk::Label,
    age_ms: u64,
    expires_at: Option<Instant>,
    reason: String,
    pin_slot: u32,
    listed_at: Instant,
}

impl ItemRow {
    /// Builds the row for `item`, numbered `number` when it is among the first nine.
    #[must_use]
    pub fn new(item: &ItemSummary, number: Option<usize>, listed_at: Instant) -> Self {
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

        let content = gtk::Stack::new();
        content.set_transition_type(gtk::StackTransitionType::Crossfade);
        content.set_transition_duration(170);
        content.set_hhomogeneous(false);
        let revealed = content_label("");
        if masked {
            let shape = if item.sensitivity == sensitivity::SECRET {
                Shape::Solid
            } else {
                Shape::Hatch
            };
            content.add_named(&mask::masked_text(shape), Some("mask"));
        } else if item.kind == kind::IMAGE {
            let label = content_label(&format!("Image · {}", item.mime));
            label.add_css_class("dim-label");
            content.add_named(&label, Some("mask"));
        } else {
            content.add_named(&content_label(&first_line(&item.preview)), Some("mask"));
        }
        content.add_named(&revealed, Some("revealed"));
        content.set_visible_child_name("mask");
        body.append(&content);

        let meta = gtk::Label::new(None);
        meta.add_css_class("item-meta");
        meta.add_css_class("caption");
        meta.set_xalign(0.0);
        body.append(&meta);
        line.append(&body);

        if item.pin_slot > 0 {
            let pin = gtk::Image::from_icon_name("view-pin-symbolic");
            pin.add_css_class("item-pin");
            pin.set_tooltip_text(Some(&format!("Pin slot {}", item.pin_slot)));
            line.append(&pin);
        }

        let widget = gtk::ListBoxRow::builder().child(&line).build();
        widget.update_property(&[gtk::accessible::Property::Label(&text::accessible_label(
            item,
        ))]);

        let row = Self {
            id: item.id,
            widget,
            masked,
            content,
            revealed,
            meta,
            age_ms: item.age_ms,
            expires_at: (item.expires_in_ms > 0)
                .then(|| listed_at + Duration::from_millis(item.expires_in_ms)),
            reason: item.reason.clone(),
            pin_slot: item.pin_slot,
            listed_at,
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

    /// Shows `value` in place of the Mask until [`ItemRow::conceal`].
    pub fn reveal(&self, value: &str) {
        self.revealed.set_text(&first_line(value));
        self.content.set_visible_child_name("revealed");
    }

    /// Puts the Mask back and forgets the revealed value.
    pub fn conceal(&self) {
        self.content.set_visible_child_name("mask");
        self.revealed.set_text("");
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
