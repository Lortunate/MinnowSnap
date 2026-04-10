#!/usr/bin/env python3
"""Fail CI if the desktop app dependency tree reintroduces Qt runtime crates."""

from __future__ import annotations

import re
import subprocess
import sys

BANNED_EXACT = {
    "qmetaobject",
    "qmetaobject_impl",
    "qttypes",
    "qt_ritual_build",
    "qt_core",
    "qt_gui",
    "qt_widgets",
}

BANNED_PREFIXES = ("qt_",)
BANNED_PATTERNS = (re.compile(r"qmetaobject", re.IGNORECASE),)


def main() -> int:
    cmd = ["cargo", "tree", "-e", "normal", "-p", "minnow-app"]
    try:
        raw_output = subprocess.check_output(cmd, text=False, stderr=subprocess.STDOUT)
    except subprocess.CalledProcessError as exc:
        failed_output = exc.output.decode("utf-8", errors="replace")
        print(failed_output, end="")
        print(f"error: failed to execute {' '.join(cmd)}", file=sys.stderr)
        return exc.returncode or 1
    output = raw_output.decode("utf-8", errors="replace")

    offenders: set[str] = set()
    for line in output.splitlines():
        match = re.search(r"([A-Za-z0-9_.+-]+)\s+v\d", line)
        if not match:
            continue
        crate = match.group(1)

        if crate in BANNED_EXACT or any(crate.startswith(prefix) for prefix in BANNED_PREFIXES):
            offenders.add(crate)
            continue

        if any(pattern.search(crate) for pattern in BANNED_PATTERNS):
            offenders.add(crate)

    if offenders:
        joined = ", ".join(sorted(offenders))
        print(f"error: detected legacy Qt runtime crates in dependency graph: {joined}", file=sys.stderr)
        return 1

    print("ok: no legacy Qt runtime crates detected in minnow-app dependency graph")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
