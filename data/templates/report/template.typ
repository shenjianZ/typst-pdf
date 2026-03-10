#let template(content, show_toc: true) = {
  set text(size: 10.8pt, fill: rgb("#111827"))
  set par(justify: false, leading: 0.82em, spacing: 0.62em)
  set block(spacing: 1.05em)
  set list(spacing: 0.38em)
  set enum(spacing: 0.38em)

  show heading.where(level: 1): set text(21pt, weight: "bold", fill: rgb("#0f172a"))
  show heading.where(level: 1): set block(above: 1.4em, below: 0.9em)
  show heading.where(level: 2): set text(15pt, weight: "bold", fill: rgb("#0f172a"))
  show heading.where(level: 2): set block(above: 1.15em, below: 0.65em)
  show heading.where(level: 3): set text(12.5pt, weight: "semibold", fill: rgb("#1e293b"))
  show heading.where(level: 3): set block(above: 0.95em, below: 0.55em)
  show link: set text(fill: rgb("#0f766e"))
  show raw.where(block: false): box.with(
    inset: (x: 0.28em, y: 0.12em),
    radius: 3pt,
    fill: rgb("#f3f4f6"),
    stroke: 0.6pt + rgb("#d1d5db"),
  )
  show raw.where(block: true): block.with(
    width: 100%,
    inset: (x: 11pt, y: 10pt),
    radius: 6pt,
    fill: rgb("#f8fafc"),
    stroke: 0.8pt + rgb("#d8dee8"),
    above: 0.7em,
    below: 0.95em,
  )
  show math.equation: set block(above: 0.75em, below: 0.95em)

  show figure: set align(center)
  show figure: set block(above: 1em, below: 1.1em)
  show figure.caption: set text(9pt, fill: rgb("#64748b"))

  [
    #align(center)[
      #block(
        below: 1.4em,
      )[
        #text(21pt, weight: "bold", fill: rgb("#0f172a"))[Document]
        #v(0.35em)
        #text(9pt, fill: rgb("#64748b"))[Generated from Markdown]
      ]
    ]
    #if show_toc [
      #block[
        #outline(indent: 1.6em)
      ]
      #pagebreak()
    ]
    #block(
      inset: (x: 2pt, y: 0pt),
    )[
      #content
    ]
  ]
}
