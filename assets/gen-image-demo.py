#!/usr/bin/env python3
"""Generate image-mode demo GIF for termula README.

Creates an animated GIF showing termula's image rendering mode
(Kitty/iTerm2) by compositing typst-rendered math PNGs into
terminal-style frames.

Requirements: typst, agg, Python 3
Usage: python3 assets/gen-image-demo.py
"""

import json
import os
import subprocess
import tempfile

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPT_DIR)
CAST_FILE = os.path.join(SCRIPT_DIR, "demo-image.cast")
GIF_FILE = os.path.join(SCRIPT_DIR, "demo-image.gif")

# Scenes: (typed command, math typst source, prefix text)
SCENES = [
    (
        r"echo '$$\frac{-b \pm \sqrt{b^2-4ac}}{2a}$$' | termula",
        r"$ frac(-b plus.minus sqrt(b^2 - 4a c), 2a) $",
        "The quadratic formula:\n",
    ),
    (
        r"echo '$$\int_0^1 x^2 dx = \frac{1}{3}$$' | termula",
        r"$ integral_0^1 x^2 thin d x = frac(1, 3) $",
        "",
    ),
    (
        r"echo '$$\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$$' | termula",
        r"$ sum_(i=1)^n i = frac(n(n+1), 2) $",
        "",
    ),
    (
        r"echo 'Einstein: $$E = mc^2$$' | termula",
        r"$ E = m c^2 $",
        "Einstein: ",
    ),
]


def render_math_png(typst_src, output_path):
    """Render a typst math expression to PNG."""
    with tempfile.NamedTemporaryFile(suffix=".typ", mode="w", delete=False) as f:
        f.write(
            '#set page(width: auto, height: auto, margin: (x: 6pt, y: 6pt), fill: rgb("1e1e1e"))\n'
        )
        f.write("#set text(fill: white, size: 18pt)\n")
        f.write(typst_src + "\n")
        f.flush()
        subprocess.run(
            ["typst", "compile", "--ppi", "144", f.name, output_path],
            capture_output=True,
        )
        os.unlink(f.name)


def build_composite_typst(scenes_data, output_typ, output_dir):
    """Build a multi-page typst doc where each page is an animation frame."""
    # First render all math PNGs
    math_pngs = []
    for i, (_, typst_src, _) in enumerate(scenes_data):
        png_path = os.path.join(output_dir, f"math{i}.png")
        render_math_png(typst_src, png_path)
        math_pngs.append(png_path)

    lines = []
    lines.append('#set page(width: 660pt, height: 520pt, margin: (x: 0pt, y: 0pt), fill: rgb("1e1e1e"))')
    lines.append('#set text(fill: white, font: "DejaVu Sans Mono", size: 10.5pt)')
    lines.append("")

    # Terminal box helper
    lines.append("""
#let terminal-box(content) = {
  block(
    width: 100%,
    height: 100%,
    fill: rgb("282828"),
    radius: 8pt,
    clip: true,
    {
      block(
        width: 100%,
        fill: rgb("3c3c3c"),
        inset: (x: 12pt, y: 8pt),
        {
          text(fill: rgb("ff5f57"), size: 10pt)[●]
          h(4pt)
          text(fill: rgb("febc2e"), size: 10pt)[●]
          h(4pt)
          text(fill: rgb("28c840"), size: 10pt)[●]
          h(12pt)
          text(fill: rgb("999999"), size: 10pt)[Terminal — Image Mode (Kitty/iTerm2)]
        }
      )
      block(
        width: 100%,
        inset: (x: 16pt, y: 12pt),
        content
      )
    }
  )
}

#let prompt-line(cmd) = {
  text(fill: rgb("50fa7b"))[\\$] + [ ]
  text(fill: white)[#cmd]
  linebreak()
}
""")

    # Generate frames progressively:
    # For each scene, generate:
    #   1. Frame with command being typed (partial)
    #   2. Frame with command complete
    #   3. Frame with output appearing
    #   4. Hold frame

    frame_count = 0

    def emit_page(content_lines):
        nonlocal frame_count
        if frame_count > 0:
            lines.append("#pagebreak()")
        lines.append("#terminal-box({")
        for cl in content_lines:
            lines.append(f"  {cl}")
        lines.append("})")
        frame_count += 1

    accumulated = []  # lines that persist across scenes

    for scene_idx, (cmd, _, prefix) in enumerate(scenes_data):
        png_path = math_pngs[scene_idx]

        # Typing frames - type in chunks
        chunk_size = 8
        for j in range(0, len(cmd), chunk_size):
            partial = cmd[: j + chunk_size]
            page_content = list(accumulated)
            page_content.append(
                f'text(fill: rgb("50fa7b"))[\\$] + [ ] + text(fill: white)[{escape_typst(partial)}]'
            )
            # Cursor
            page_content.append(
                'text(fill: white)[█]'
            )
            emit_page(page_content)

        # Command complete frame
        page_content = list(accumulated)
        page_content.append(f"prompt-line[{escape_typst(cmd)}]")
        emit_page(page_content)

        # Brief pause frame (same as above)
        emit_page(list(page_content))

        # Output frame with rendered image
        page_content_with_output = list(page_content)
        if prefix:
            page_content_with_output.append(f"v(4pt)")
            page_content_with_output.append(
                f'text(fill: rgb("e0e0e0"))[{escape_typst(prefix)}]'
            )
        page_content_with_output.append("v(4pt)")
        # Use relative path from the typst file's directory
        rel_png = os.path.basename(png_path)
        page_content_with_output.append(
            f'image("{rel_png}", height: 50pt)'
        )
        emit_page(page_content_with_output)

        # Hold frames (3 frames to pause)
        for _ in range(4):
            emit_page(list(page_content_with_output))

        # Accumulate for next scene
        accumulated = list(page_content_with_output)
        accumulated.append("v(8pt)")

    # Final hold
    for _ in range(5):
        emit_page(list(accumulated))

    with open(output_typ, "w") as f:
        f.write("\n".join(lines))

    return frame_count


def escape_typst(s):
    """Escape string for typst markup content."""
    s = s.replace("\\", "\\\\")
    s = s.replace('"', '\\"')
    s = s.replace("$", "\\$")
    s = s.replace("#", "\\#")
    s = s.replace("{", "\\{")
    s = s.replace("}", "\\}")
    s = s.replace("_", "\\_")
    s = s.replace("^", "\\^")
    s = s.replace("@", "\\@")
    s = s.replace("<", "\\<")
    s = s.replace(">", "\\>")
    return s


def main():
    with tempfile.TemporaryDirectory() as tmpdir:
        typ_file = os.path.join(tmpdir, "frames.typ")
        png_pattern = os.path.join(tmpdir, "frame{0p}.png")

        print("Building composite typst document...")
        n_frames = build_composite_typst(SCENES, typ_file, tmpdir)
        print(f"  {n_frames} frames")

        print("Compiling frames with typst...")
        # typst {0p} outputs 1-indexed filenames like frame1.png, frame2.png, ...
        png_out = os.path.join(tmpdir, "frame{0p}.png")
        subprocess.run(
            ["typst", "compile", "--ppi", "144", typ_file, png_out],
            check=True,
        )

        # Rename to 0-indexed sequential for ffmpeg
        pngs = sorted(
            [f for f in os.listdir(tmpdir) if f.startswith("frame") and f.endswith(".png") and f != "frames.typ"]
        )
        print(f"  {len(pngs)} frame images generated")

        if not pngs:
            print("ERROR: No frames generated!")
            return

        # Rename to sequential 0-indexed: out0000.png, out0001.png, ...
        for idx, png_name in enumerate(pngs):
            old = os.path.join(tmpdir, png_name)
            new = os.path.join(tmpdir, f"out{idx:04d}.png")
            os.rename(old, new)

        # Use ffmpeg to create GIF
        print("Creating GIF with ffmpeg...")
        subprocess.run(
            [
                "ffmpeg",
                "-framerate", "6",
                "-i", os.path.join(tmpdir, "out%04d.png"),
                "-vf", "split[s0][s1];[s0]palettegen=max_colors=64[p];[s1][p]paletteuse=dither=bayer",
                "-loop", "0",
                "-y",
                GIF_FILE,
            ],
            check=True,
            capture_output=True,
        )

        print(f"Done! {GIF_FILE}")
        size = os.path.getsize(GIF_FILE)
        print(f"  Size: {size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
