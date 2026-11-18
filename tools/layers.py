#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check that each crate depends only on what its layer allows.

    python3 tools/layers.py

The rules below are the architecture in docs/engineering.md, made checkable.
Only normal (non-dev) dependencies count. Changing a rule is an architecture
decision: say why in the PR.

Standard library only, so CI and contributors need nothing extra.
"""

import json
import subprocess
import sys

# For each crate:
#   "workspace": the workspace crates it may depend on.
#   "external":  the external crates it may depend on, or None for any.
# The security crates list their external dependencies one by one, so adding
# one means changing this file, which CODEOWNERS routes to a maintainer.
RULES = {
    # Pure detection: no I/O, no clock, no allocation of secrets beyond its input.
    "copyveil-detect": {"workspace": set(), "external": {"regex"}},
    # History, Sensitivity, encryption and the store. No D-Bus, no async, no desktop.
    "copyveil-core": {
        "workspace": {"copyveil-detect"},
        "external": {"chacha20poly1305", "rusqlite", "zeroize"},
    },
    # The one place that talks to the keyring.
    "copyveil-keyring": {
        "workspace": {"copyveil-core"},
        "external": {"oo7", "zeroize"},
    },
    # The D-Bus contract: names, the Item summary and client proxies. No
    # logic and no Item content, so clients can share it with the daemon.
    "copyveil-dbus": {"workspace": set(), "external": {"serde", "zbus"}},
    # Glue: D-Bus, the async runtime, files, process setup.
    "copyveil-daemon": {
        "workspace": {"copyveil-core", "copyveil-keyring", "copyveil-dbus"},
        "external": None,
    },
    # The app and capture sources reach the History only over D-Bus.
    "copyveil-app": {"workspace": {"copyveil-dbus"}, "external": None},
    "copyveil-capture-datacontrol": {"workspace": set(), "external": None},
}

# Crates no layer below the daemon may use, whatever the allowlists say.
NEVER_IN_CORE = {"zbus", "tokio", "gtk4", "libadwaita", "relm4", "oo7"}


def main():
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    workspace = {package["name"] for package in metadata["packages"]}
    problems = []

    for name in sorted(workspace - RULES.keys()):
        problems.append(f"{name}: no layer rule; add it to tools/layers.py")

    for package in metadata["packages"]:
        rule = RULES.get(package["name"])
        if rule is None:
            continue
        deps = {dep["name"] for dep in package["dependencies"] if dep["kind"] is None}
        for dep in sorted(deps & workspace - rule["workspace"]):
            problems.append(f"{package['name']} may not depend on {dep}")
        external = deps - workspace
        if rule["external"] is not None:
            for dep in sorted(external - rule["external"]):
                problems.append(
                    f"{package['name']}: {dep} is not on its dependency allowlist"
                )
        if package["name"] in {"copyveil-detect", "copyveil-core"}:
            for dep in sorted(external & NEVER_IN_CORE):
                problems.append(f"{package['name']} may not depend on {dep}")

    for problem in problems:
        print(f"layers: {problem}", file=sys.stderr)
    if problems:
        print("See docs/engineering.md#architecture.", file=sys.stderr)
        return 1
    print(f"layers: {len(RULES)} crates follow their layer rules")
    return 0


if __name__ == "__main__":
    sys.exit(main())
