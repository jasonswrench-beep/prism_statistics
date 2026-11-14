// The layout of a PrismStatistics PDF report. The content comes from data.json, written by lib.rs from the result
// bundles, so no user text is ever parsed as Typst markup. Exported as PDF/UA-1: headings, tables with header rows,
// lists and paragraphs are tagged, the language is set, and the document has a title.

#let data = json("data.json")

#set document(title: data.title, author: "PrismStatistics")
#set text(font: "Libertinus Serif", size: 11pt, lang: "en")
#set page(paper: "us-letter", margin: 1in, numbering: "1")
#set par(justify: false)
#show heading.where(level: 1): set text(size: 17pt)
#show heading.where(level: 2): it => { v(14pt); set text(size: 13pt); it }
#show heading.where(level: 3): set text(size: 11pt)

// Text with italic and subscript runs, as the template writer and the table headings mark them.
#let runs(rs) = rs.map(r => if r.italic { emph(r.text) } else if r.sub { sub(r.text) } else { r.text }).join()

#let meta(t) = text(size: 9.5pt, fill: rgb("#555555"), t)

#let warning(t) = block(stroke: (left: 2pt + rgb("#B45309")), inset: (left: 8pt, y: 4pt), width: 100%, t)

// A table stays on one page with its label and title unless it is too long to fit, as APA asks.
#let apa-table(t) = block(breakable: t.rows.len() > 30, width: 100%)[
  #strong[Table #t.number] \
  #emph(t.title)
  #set text(size: 9.5pt)
  #table(
    columns: t.columns.len(),
    stroke: none,
    inset: (x: 5pt, y: 3pt),
    align: (x, y) => if x == 0 { left } else { center },
    table.hline(stroke: 1pt),
    table.header(..t.columns.map(c => runs(c))),
    table.hline(stroke: 0.5pt),
    ..t.rows.flatten(),
    table.hline(stroke: 1pt),
  )
  #if t.note != none [#emph[Note.] #t.note]
]

// A figure: bold number, italic title, then the chart (with alternative text for screen readers), kept together.
#let apa-figure(f) = block(breakable: false, width: 100%)[
  #strong[Figure #f.number] \
  #emph(f.title)
  #v(4pt)
  #image(bytes(f.svg), format: "svg", alt: f.alt, width: 100%)
]

// The writer's own headings and paragraphs (Report Builder).
#let own(b) = if b.heading [== #runs(b.runs)] else [#par(leading: 1em, first-line-indent: (amount: 0.5in, all: true), runs(b.runs))]

= #data.title
#meta(data.subtitle)

#for r in data.results [
  #for b in r.before [#own(b)]
  == #r.title
  #meta(r.cases)

  #for w in r.warnings [#warning(w)]

  #for f in r.figures [#apa-figure(f)]

  #for t in r.tables [#apa-table(t)]

  #if r.checks.len() > 0 [
    === Checks
    #for c in r.checks [- #strong[#c.title:] #c.state. #c.message]
  ]

  #if r.paragraph != none [
    === #r.paragraph_title
    #par(leading: 1em, first-line-indent: (amount: 0.5in, all: true), runs(r.paragraph))
  ]

  #if r.method != "" [#meta[#strong[Method.] #r.method]]
]

#for b in data.after [#own(b)]
