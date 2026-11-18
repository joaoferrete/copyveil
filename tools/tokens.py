#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate GTK stylesheets from design/tokens.json and check WCAG contrast.

    python3 tools/tokens.py           regenerate data/ui/style*.css and the app's tokens.rs
    python3 tools/tokens.py --check   fail if generated files are stale or contrast fails

Standard library only, so CI and contributors need nothing extra.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOKENS = ROOT / "design" / "tokens.json"
OUT = {"light": ROOT / "data" / "ui" / "style.css", "dark": ROOT / "data" / "ui" / "style-dark.css"}
HEADER = "/* Generated from design/tokens.json by tools/tokens.py. Do not edit. */\n"


def colour_tokens(tokens):
    for group in ("color", "sensitivity"):
        for name, value in tokens[group].items():
            yield group, name, value


def render(tokens, theme):
    lines = [HEADER]
    for group, name, value in colour_tokens(tokens):
        prefix = "cv" if group == "color" else "cv-sensitivity"
        lines.append(f"@define-color {prefix}_{name.replace('-', '_')} {value[theme]};\n")
    return "".join(lines)


RUST = ROOT / "crates" / "copyveil-app" / "src" / "tokens.rs"


def render_rust(tokens):
    """Colours as Rust constants, for drawing that can't use CSS (Pango
    attributes). Each is `(light, dark)`, as `(r, g, b)` from 0 to 1."""
    def rgb(hex_colour):
        hex_colour = hex_colour.lstrip("#")
        channels = [int(hex_colour[i : i + 2], 16) / 255 for i in (0, 2, 4)]
        return "(" + ", ".join(f"{c:.4f}_f32" for c in channels) + ")"

    lines = [
        "//! Generated from design/tokens.json by tools/tokens.py. Do not edit.\n",
        "//!\n",
        "//! Colours for drawing that can't go through CSS. Each is `(light, dark)`,\n",
        "//! as `(red, green, blue)` from 0 to 1.\n\n",
        "/// A colour in both themes.\n",
        "pub type Themed = ((f32, f32, f32), (f32, f32, f32));\n",
    ]
    for group, name, value in colour_tokens(tokens):
        const = (name if group == "color" else f"sensitivity-{name}").replace("-", "_").upper()
        # Code names in a role (ExpiryRing) go in backticks, as rustdoc wants.
        role = re.sub(r"\b([A-Z][a-z]+[A-Z]\w*)", r"`\1`", value.get("role", ""))
        lines.append(f"\n/// {role}\n")
        lines.append(f"pub const {const}: Themed = ({rgb(value['light'])}, {rgb(value['dark'])});\n")
    return "".join(lines)


def luminance(hex_colour):
    hex_colour = hex_colour.lstrip("#")
    channels = [int(hex_colour[i : i + 2], 16) / 255 for i in (0, 2, 4)]
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in channels]
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]


def contrast(a, b):
    hi, lo = sorted((luminance(a), luminance(b)), reverse=True)
    return (hi + 0.05) / (lo + 0.05)


def lookup(tokens, path):
    group, name = path.split(".", 1)
    return tokens[group][name]


def check_contrast(tokens):
    failures = []
    for rule in tokens["contrast"]:
        for theme in ("light", "dark"):
            fg = lookup(tokens, rule["fg"])[theme]
            bg = lookup(tokens, rule["bg"])[theme]
            ratio = contrast(fg, bg)
            if ratio < rule["min"]:
                failures.append(f"{theme}: {rule['fg']} on {rule['bg']} is {ratio:.2f}:1, needs {rule['min']}:1")
    return failures


def main():
    check = "--check" in sys.argv[1:]
    tokens = json.loads(TOKENS.read_text(encoding="utf-8"))
    problems = check_contrast(tokens)

    for theme, path in OUT.items():
        css = render(tokens, theme)
        if check:
            if not path.exists() or path.read_text(encoding="utf-8") != css:
                problems.append(f"{path.relative_to(ROOT)} is stale; run python3 tools/tokens.py")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(css, encoding="utf-8")

    rust = render_rust(tokens)
    if check:
        if not RUST.exists() or RUST.read_text(encoding="utf-8") != rust:
            problems.append(f"{RUST.relative_to(ROOT)} is stale; run python3 tools/tokens.py")
    else:
        RUST.write_text(rust, encoding="utf-8")

    for problem in problems:
        sys.stderr.write(f"tokens: {problem}\n")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
