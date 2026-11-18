//! Generated from design/tokens.json by tools/tokens.py. Do not edit.
//!
//! Colours for drawing that can't go through CSS. Each is `(light, dark)`,
//! as `(red, green, blue)` from 0 to 1.

/// A colour in both themes.
pub type Themed = ((f32, f32, f32), (f32, f32, f32));

/// Primary text
pub const INK: Themed = ((0.0980_f32, 0.1098_f32, 0.1333_f32), (0.9059_f32, 0.9137_f32, 0.9294_f32));

/// Secondary text, metadata
pub const INK_2: Themed = ((0.3412_f32, 0.3647_f32, 0.4118_f32), (0.6039_f32, 0.6314_f32, 0.6824_f32));

/// Content surface behind Items
pub const SHEET: Themed = ((1.0000_f32, 1.0000_f32, 1.0000_f32), (0.0941_f32, 0.1059_f32, 0.1294_f32));

/// Brand. `ExpiryRing`, onboarding, focus within brand surfaces
pub const SLATE: Themed = ((0.2039_f32, 0.2745_f32, 0.3725_f32), (0.6549_f32, 0.7216_f32, 0.8275_f32));

/// Brand tint surface (onboarding art)
pub const SLATE_SOFT: Themed = ((0.8627_f32, 0.8902_f32, 0.9255_f32), (0.1451_f32, 0.1882_f32, 0.2588_f32));

/// Security-tint pattern lines
pub const TINT: Themed = ((0.5490_f32, 0.6118_f32, 0.7020_f32), (0.3686_f32, 0.4314_f32, 0.5333_f32));

/// App icon only
pub const KRAFT: Themed = ((0.7686_f32, 0.6039_f32, 0.3608_f32), (0.7882_f32, 0.6353_f32, 0.4157_f32));

/// Search match highlight only
pub const MARKER: Themed = ((0.9451_f32, 0.8745_f32, 0.4549_f32), (0.8863_f32, 0.8157_f32, 0.4078_f32));

/// Text on marker
pub const ON_MARKER: Themed = ((0.0980_f32, 0.1098_f32, 0.1333_f32), (0.0980_f32, 0.1098_f32, 0.1333_f32));

/// Secret Mask: solid bar, fixed width
pub const SENSITIVITY_SECRET: Themed = ((0.1490_f32, 0.1686_f32, 0.2078_f32), (0.8353_f32, 0.8549_f32, 0.8902_f32));

/// Suspected Mask: 45deg hatch, edges of the value visible
pub const SENSITIVITY_SUSPECTED: Themed = ((0.1490_f32, 0.1686_f32, 0.2078_f32), (0.6824_f32, 0.7176_f32, 0.7765_f32));
