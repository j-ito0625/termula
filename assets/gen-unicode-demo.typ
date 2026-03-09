// Unicode art mode demo
#set page(width: 600pt, height: auto, margin: (x: 24pt, y: 20pt), fill: rgb("1e1e1e"))
#set text(fill: white, font: "DejaVu Sans Mono", size: 11pt)

#let terminal-box(title, content) = {
  block(
    width: 100%,
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
          text(fill: rgb("999999"), size: 10pt)[#title]
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

#let green(t) = text(fill: rgb("50fa7b"))[#t]
#let gray(t) = text(fill: rgb("888888"))[#t]
#let yellow(t) = text(fill: rgb("f1fa8c"))[#t]

#terminal-box("Terminal — Unicode Art Mode (works in any terminal)", {
  text(fill: rgb("50fa7b"))[\$] + [ ]
  text(fill: white)[echo '\$\$\\frac\{-b \\pm \\sqrt\{b\^2 - 4ac\}\}\{2a\}\$\$' | termula]
  linebreak()
  v(4pt)
  text(size: 12pt, fill: rgb("e0e0e0"))[
    ┌────────┐#linebreak()
    -b ±╲│b² - 4ac#linebreak()
    ───────────────#linebreak()
    #h(36pt)2a
  ]
  v(12pt)
  text(fill: rgb("50fa7b"))[\$] + [ ]
  text(fill: white)[echo '\$\$\\int\_0\^1 x\^2 dx = \\frac\{1\}\{3\}\$\$' | termula]
  linebreak()
  v(4pt)
  text(size: 12pt, fill: rgb("e0e0e0"))[
    ⌠¹ #h(36pt) 1#linebreak()
    ⎮  x² dx = ─#linebreak()
    ⌡₀ #h(36pt) 3
  ]
  v(12pt)
  text(fill: rgb("50fa7b"))[\$] + [ ]
  text(fill: white)[echo '\$\$\\sum\_\{i=1\}\^\{n\} i = \\frac\{n(n+1)\}\{2\}\$\$' | termula]
  linebreak()
  v(4pt)
  text(size: 12pt, fill: rgb("e0e0e0"))[
    #h(2pt)ₙ#linebreak()
    #h(2pt)⎲ #h(18pt) n(n+1)#linebreak()
    #h(2pt)⎳  i = ──────#linebreak()
    ⁱ⁼¹ #h(24pt) 2
  ]
})
