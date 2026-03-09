#!/usr/bin/env python3
"""Generate asciinema .cast file for termula demo GIF."""

import json
import subprocess
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPT_DIR)
TERMULA = os.path.join(PROJECT_DIR, "target", "release", "termula")
CAST_FILE = os.path.join(SCRIPT_DIR, "demo.cast")


def run_termula(latex_input):
    """Run termula and capture output."""
    result = subprocess.run(
        [TERMULA, "-m", "unicode"],
        input=latex_input,
        capture_output=True,
        text=True,
    )
    return result.stdout


def main():
    # Build release if needed
    if not os.path.exists(TERMULA):
        subprocess.run(
            ["cargo", "build", "--release"],
            cwd=PROJECT_DIR,
            capture_output=True,
        )

    # Generate rendered outputs
    formulas = {
        "quadratic": r"$$\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$",
        "integral": r"$$\int_0^1 x^2 dx = \frac{1}{3}$$",
        "summation": r"$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$",
        "einstein": r"$$E = mc^2$$",
    }

    outputs = {}
    for key, latex in formulas.items():
        outputs[key] = run_termula(latex)

    # Commands to display (shortened for readability)
    commands = [
        (
            r"echo '$$\frac{-b \pm \sqrt{b^2-4ac}}{2a}$$' | termula",
            outputs["quadratic"],
            None,
        ),
        (
            r"echo '$$\int_0^1 x^2 dx = \frac{1}{3}$$' | termula",
            outputs["integral"],
            None,
        ),
        (
            r"echo '$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$' | termula",
            outputs["summation"],
            None,
        ),
        (
            r"echo 'Einstein: $$E = mc^2$$' | termula",
            outputs["einstein"],
            "Einstein: ",
        ),
    ]

    events = []
    t = 0.0

    def add(dt, text):
        nonlocal t
        t += dt
        if text:  # skip empty strings
            events.append([round(t, 4), "o", text])

    def type_text(text, char_dt=0.04):
        for ch in text:
            add(char_dt, ch)

    def emit_lines(text, prefix=""):
        lines = text.split("\n")
        for i, line in enumerate(lines):
            out = prefix + line if i == 0 and prefix else line
            add(0.02, out + "\r\n")

    # Header
    header = {
        "version": 2,
        "width": 72,
        "height": 24,
        "timestamp": 1700000000,
        "env": {"SHELL": "/bin/bash", "TERM": "xterm-256color"},
    }

    # Generate scenes
    for i, (cmd, output, prefix) in enumerate(commands):
        if i > 0:
            add(0.3, "\r\n")

        # Green prompt
        add(0.2, "\x1b[1;32m$ \x1b[0m")
        add(0.1, "")

        # Type command
        type_text(cmd)

        # Press enter
        add(0.3, "\r\n")
        add(0.15, "")

        # Show output
        emit_lines(output, prefix=prefix or "")

        # Pause to read
        add(1.5, "")

    # Final pause
    add(1.0, "")

    # Write cast file
    with open(CAST_FILE, "w") as f:
        f.write(json.dumps(header) + "\n")
        for event in events:
            f.write(json.dumps(event) + "\n")

    print(f"Generated {CAST_FILE} ({len(events)} events, {t:.1f}s total)")


if __name__ == "__main__":
    main()
