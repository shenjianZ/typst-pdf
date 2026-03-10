#let template(content, show_toc: false) = {
  set text(size: 11pt, fill: rgb("#1f2937"))
  set par(justify: false, leading: 0.98em, spacing: 0.72em)
  set block(spacing: 1.12em)
  set list(spacing: 0.68em, tight: false)
  set enum(spacing: 0.68em, tight: false)

  show heading.where(level: 1): set text(23pt, weight: "bold", fill: rgb("#111827"))
  show heading.where(level: 1): set block(above: 0.3em, below: 0.72em)
  show heading.where(level: 2): set text(18pt, weight: "bold", fill: rgb("#111827"))
  show heading.where(level: 2): set block(above: 1.38em, below: 0.62em)
  show heading.where(level: 3): set text(14pt, weight: "semibold", fill: rgb("#374151"))
  show heading.where(level: 3): set block(above: 1em, below: 0.46em)
  show heading.where(level: 4): set text(11.6pt, weight: "semibold", fill: rgb("#374151"))
  show heading.where(level: 5): set text(10.4pt, weight: "medium", fill: rgb("#4b5563"))
  show heading.where(level: 6): set text(9.6pt, weight: "medium", fill: rgb("#6b7280"))

  show link: set text(fill: rgb("#2563eb"))
  show strong: set text(weight: "bold", fill: rgb("#111827"))
  show emph: set text(style: "italic", fill: rgb("#4b5563"))

  show raw.where(block: false): box.with(
    inset: (x: 0.28em, y: 0.12em),
    radius: 3pt,
    fill: rgb("#f3f4f6"),
    stroke: 0.55pt + rgb("#d9dee7"),
  )
  show raw.where(block: true): set text(size: 9.8pt, fill: rgb("#334155"))

  show figure: set block(above: 1em, below: 1.1em)
  show figure.caption: set text(size: 8.8pt, fill: rgb("#6b7280"))
  show math.equation: set block(above: 0.65em, below: 0.85em)

  [
    #if show_toc [
      #block(
        inset: (x: 12pt, y: 10pt),
        fill: rgb("#f8fafc"),
        stroke: 0.7pt + rgb("#e2e8f0"),
        radius: 4pt,
        below: 1.2em,
      )[
        #set text(size: 9pt, fill: rgb("#475569"))
        #outline(indent: 1.5em)
      ]
    ]
    #content
  ]
}
