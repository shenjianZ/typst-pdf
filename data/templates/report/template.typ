#let template(content, show_toc: true) = {
  set text(size: 11pt)
  set par(justify: true, leading: 0.72em)

  [
    #align(center, text(20pt, weight: "bold")[Document])
    #v(1em)
    #if show_toc [
      #outline(indent: 1.5em)
      #pagebreak()
    ]
    #content
  ]
}
