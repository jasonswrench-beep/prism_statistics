//! Word (.docx) and HTML export of results (task T3-05 subset for the teaching preview).
//!
//! Both formats are written from the result bundles, the same data the Output tab shows, so an exported number can't
//! differ from the screen. Tables follow APA style: a bold "Table n" label, an italic title, horizontal rules only, and
//! notes beginning with an italic *Note.* Every result keeps its case accounting, checks and method note, and a result
//! that may not be interpreted says why instead of showing a paragraph.
//!
//! The .docx is written directly as Office Open XML (a zip of a few XML parts), with no template and no program
//! metadata. The HTML is one self-contained file with semantic tables (caption, column headers) for screen readers.

use std::io::{Cursor, Write};

use crate::apa;
use crate::error::{Error, Result};
use crate::result::{Cell, DiagnosticState, ResultBundle, Status, Table};

/// One result to export.
pub struct Item<'a> {
    pub bundle: &'a ResultBundle,
    pub paragraph: Option<&'a str>,
    pub stale: bool,
    /// When the paragraph is an AI draft the user reviewed, the model that wrote it.
    pub ai_model: Option<&'a str>,
    /// A PNG of each of the bundle's charts, in order, for Word (empty where none could be drawn).
    pub images: Vec<Vec<u8>>,
    /// Which parts of the result to show (the Report Builder can leave some out).
    pub parts: Parts,
    /// The writer's headings and text placed before this result.
    pub before: Vec<Block>,
}

/// The parts of a result an export shows. All of them unless the Report Builder says otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parts {
    pub charts: bool,
    pub tables: bool,
    pub checks: bool,
    pub paragraph: bool,
    pub method: bool,
}

impl Default for Parts {
    fn default() -> Self {
        Parts { charts: true, tables: true, checks: true, paragraph: true, method: true }
    }
}

/// A heading or a paragraph of the writer's own.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub heading: bool,
    pub text: String,
}

/// The heading over a result's paragraph.
pub fn paragraph_heading(item: &Item) -> String {
    match item.ai_model {
        Some(m) => format!("Results paragraph (AI-assisted with {m}, reviewed)"),
        None => "Results paragraph (template)".to_string(),
    }
}

/// What goes at the top of the document.
pub struct Header {
    pub title: String,
    /// Where the data came from, e.g. "survey.xlsx (sheet Responses)".
    pub source: Option<String>,
    /// The date, written by the caller so exports are reproducible in tests.
    pub date: String,
    /// The writer's headings and text after the last result.
    pub after: Vec<Block>,
}

/// A table cell as text, the way the Output tab writes it.
pub fn cell_text(c: &Cell) -> String {
    match c {
        Cell::Text { text } => text.clone(),
        Cell::Number { value, decimals } => {
            if value.is_finite() {
                apa::num(*value, *decimals as usize)
            } else {
                "—".into()
            }
        }
        Cell::Count { value } => value.to_string(),
        Cell::P { value } => {
            if value.is_finite() {
                apa::p_cell(*value)
            } else {
                "—".into()
            }
        }
        Cell::Empty => String::new(),
    }
}

/// A piece of text with its style.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Run {
    pub text: String,
    pub italic: bool,
    pub sub: bool,
}

/// Splits text marked with `*italic*` and `<sub>subscript</sub>` into runs.
pub fn runs(text: &str) -> Vec<Run> {
    let mut out = Vec::new();
    let mut rest = text;
    let push = |out: &mut Vec<Run>, t: &str, italic: bool, sub: bool| {
        if !t.is_empty() {
            out.push(Run { text: t.to_string(), italic, sub });
        }
    };
    while !rest.is_empty() {
        let star = rest.find('*');
        let sub = rest.find("<sub>");
        match (star, sub) {
            (Some(s), b) if b.is_none_or(|b| s < b) => match rest[s + 1..].find('*') {
                Some(e) if e > 0 => {
                    push(&mut out, &rest[..s], false, false);
                    push(&mut out, &rest[s + 1..s + 1 + e], true, false);
                    rest = &rest[s + 2 + e..];
                }
                _ => {
                    push(&mut out, rest, false, false);
                    break;
                }
            },
            (_, Some(b)) => match rest[b..].find("</sub>") {
                Some(e) => {
                    push(&mut out, &rest[..b], false, false);
                    push(&mut out, &rest[b + 5..b + e], false, true);
                    rest = &rest[b + e + 6..];
                }
                None => {
                    push(&mut out, rest, false, false);
                    break;
                }
            },
            _ => {
                push(&mut out, rest, false, false);
                break;
            }
        }
    }
    out
}

/// Statistical symbols that APA sets in italics when they head a column.
const SYMBOLS: [&str; 20] = ["t", "F", "p", "df", "df1", "df2", "N", "n", "M", "SD", "SE", "r", "rho", "B", "U", "V", "H", "z", "R", "R²"];

/// A column heading with its leading statistical symbol in italics ("p (two-sided)" → *p* (two-sided)).
pub fn heading_runs(h: &str) -> Vec<Run> {
    let first = h.split([' ', '(']).next().unwrap_or("");
    if SYMBOLS.contains(&first) {
        let mut v = vec![Run { text: first.to_string(), italic: true, sub: false }];
        if h.len() > first.len() {
            v.push(Run { text: h[first.len()..].to_string(), italic: false, sub: false });
        }
        v
    } else {
        vec![Run { text: h.to_string(), italic: false, sub: false }]
    }
}

pub fn cases_line(b: &ResultBundle) -> String {
    if b.method_id == crate::analysis::power::METHOD_ID {
        return "Planning: no data used.".into();
    }
    let mut s = format!("{} of {} cases analyzed", b.cases.analyzed, b.cases.total);
    if !b.cases.excluded.is_empty() {
        s += &format!("; excluded: {}", b.cases.excluded.iter().map(|e| format!("{} ({})", e.reason, e.count)).collect::<Vec<_>>().join(", "));
    }
    s + "."
}

fn state_words(s: DiagnosticState) -> &'static str {
    s.words()
}

pub const STALE: &str = "Out of date: the data changed after this result was run. Run it again before reporting it.";

// ---------------------------------------------------------------- HTML

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html_runs(runs: &[Run]) -> String {
    runs.iter()
        .map(|r| {
            let t = esc(&r.text);
            if r.italic {
                format!("<i>{t}</i>")
            } else if r.sub {
                format!("<sub>{t}</sub>")
            } else {
                t
            }
        })
        .collect()
}

fn html_table(t: &Table, number: usize) -> String {
    let mut s = String::from("<figure class=\"apa\">\n<table>\n");
    s += &format!("<caption><b>Table {number}</b><br><i>{}</i></caption>\n<thead><tr>", esc(&t.title));
    for c in &t.columns {
        s += &format!("<th scope=\"col\">{}</th>", html_runs(&heading_runs(c)));
    }
    s += "</tr></thead>\n<tbody>\n";
    for row in &t.rows {
        s += "<tr>";
        for (i, c) in row.iter().enumerate() {
            let tag = if i == 0 { "th scope=\"row\"" } else { "td" };
            let end = if i == 0 { "th" } else { "td" };
            s += &format!("<{tag}>{}</{end}>", esc(&cell_text(c)));
        }
        s += "</tr>\n";
    }
    s += "</tbody>\n</table>\n";
    if !t.footnotes.is_empty() {
        s += &format!("<p class=\"note\"><i>Note.</i> {}</p>\n", esc(&t.footnotes.join(" ")));
    }
    s + "</figure>\n"
}

/// The results as one self-contained HTML file.
pub fn html(header: &Header, items: &[Item]) -> String {
    let mut s = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"PrismStatistics {}\">\n<title>{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<main>\n<header><h1>{}</h1><p class=\"meta\">",
        env!("CARGO_PKG_VERSION"),
        esc(&header.title),
        esc(&header.title)
    );
    if let Some(src) = &header.source {
        s += &format!("Data: {} · ", esc(src));
    }
    s += &format!("Exported {} by PrismStatistics {}</p></header>\n", esc(&header.date), env!("CARGO_PKG_VERSION"));
    let mut n = 0;
    let mut fig = 0;
    let blocks = |bl: &[Block]| bl.iter().map(|x| if x.heading { format!("<h2 class=\"own\">{}</h2>\n", esc(&x.text)) } else { format!("<p class=\"own\">{}</p>\n", html_runs(&runs(&x.text))) }).collect::<String>();
    for it in items {
        let b = it.bundle;
        s += &blocks(&it.before);
        s += &format!("<section>\n<h2>{}</h2>\n<p class=\"meta\">{}</p>\n", esc(&b.title), esc(&cases_line(b)));
        for c in b.charts.iter().filter(|_| it.parts.charts) {
            fig += 1;
            // APA: a bold figure number, the title in italic title case on the next line, then the figure.
            s += &format!("<figure class=\"apa chart\"><figcaption><b>Figure {fig}</b><br><i>{}</i></figcaption>{}</figure>\n", esc(&c.title), c.svg);
        }
        if it.stale {
            s += &format!("<p class=\"warn\" role=\"note\">{}</p>\n", esc(STALE));
        }
        if let Status::Failed { reason } = &b.status {
            s += &format!("<p class=\"warn\" role=\"note\"><b>Not interpreted.</b> {}</p>\n", esc(reason));
        }
        for t in b.tables.iter().filter(|_| it.parts.tables) {
            n += 1;
            s += &html_table(t, n);
        }
        if it.parts.checks && !b.diagnostics.is_empty() {
            s += "<h3>Checks</h3>\n<ul class=\"checks\">\n";
            for d in &b.diagnostics {
                s += &format!("<li><b>{}:</b> {}. {}</li>\n", esc(&d.title), state_words(d.state), esc(&d.message));
            }
            s += "</ul>\n";
        }
        if let Some(p) = it.paragraph.filter(|_| it.parts.paragraph) {
            s += &format!("<h3>{}</h3>\n<p class=\"para\">{}</p>\n", esc(&paragraph_heading(it)), html_runs(&runs(p)));
        }
        if it.parts.method {
            s += &format!("<p class=\"method\"><b>Method.</b> {}</p>\n", esc(&b.method_note));
        }
        s += "</section>\n";
    }
    s += &blocks(&header.after);
    s + "</main>\n</body>\n</html>\n"
}

const CSS: &str = "body{margin:0;background:#fff;color:#1b1b24;font:16px/1.5 'Times New Roman',Times,serif}main{max-width:52rem;margin:0 auto;padding:2rem 1rem}h1{font-size:1.6rem;margin:0 0 .25rem}h2{font-size:1.25rem;margin:2.5rem 0 .25rem;border-top:1px solid #ddd;padding-top:1.5rem}h3{font-size:1rem;margin:1.25rem 0 .25rem}.meta{color:#555;font-size:.9rem;margin:0 0 1rem}.warn{background:#fff4e5;border-left:3px solid #b45309;padding:.5rem .75rem}figure.apa{margin:1.25rem 0;overflow-x:auto}figure.chart svg{max-width:100%;height:auto;display:block;margin-top:.4rem}table{border-collapse:collapse;min-width:50%}caption{text-align:left;padding-bottom:.4rem;line-height:1.6}thead th{border-top:1.5px solid #000;border-bottom:1px solid #000;font-weight:normal;padding:.3rem .6rem;text-align:center}tbody td{padding:.2rem .6rem;text-align:center}tbody th{font-weight:normal;text-align:left;padding:.2rem .6rem}tbody tr:last-child>*{border-bottom:1.5px solid #000}.note{font-size:.95rem;margin:.4rem 0 0}.checks{padding-left:1.2rem}.para{background:#f7f6fb;padding:.75rem 1rem;border-radius:6px}.method{font-size:.9rem;color:#333}h2.own{border-top:none;padding-top:0}p.own{text-indent:.5in}";

// ---------------------------------------------------------------- Word

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn w_runs(runs: &[Run], bold: bool) -> String {
    runs.iter()
        .map(|r| {
            let mut rpr = String::new();
            if bold {
                rpr += "<w:b/>";
            }
            if r.italic {
                rpr += "<w:i/>";
            }
            if r.sub {
                rpr += "<w:vertAlign w:val=\"subscript\"/>";
            }
            let rpr = if rpr.is_empty() { String::new() } else { format!("<w:rPr>{rpr}</w:rPr>") };
            format!("<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>", xml(&r.text))
        })
        .collect()
}

fn plain(text: &str) -> Vec<Run> {
    vec![Run { text: text.to_string(), italic: false, sub: false }]
}

fn w_para(style: Option<&str>, body: &str) -> String {
    match style {
        Some(s) => format!("<w:p><w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>{body}</w:p>"),
        None => format!("<w:p>{body}</w:p>"),
    }
}

fn w_cell(content: &str, center: bool, borders: &str) -> String {
    let jc = if center { "<w:jc w:val=\"center\"/>" } else { "" };
    format!("<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/>{borders}</w:tcPr><w:p><w:pPr><w:pStyle w:val=\"TableText\"/>{jc}</w:pPr>{content}</w:p></w:tc>")
}

fn w_table(t: &Table, number: usize) -> String {
    let mut s = w_para(Some("TableNumber"), &w_runs(&plain(&format!("Table {number}")), true));
    s += &w_para(Some("TableTitle"), &w_runs(&[Run { text: t.title.clone(), italic: true, sub: false }], false));
    let line = |side: &str, sz: u32| format!("<w:{side} w:val=\"single\" w:sz=\"{sz}\" w:space=\"0\" w:color=\"000000\"/>");
    s += &format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders>{}{}</w:tblBorders><w:tblLayout w:type=\"autofit\"/><w:tblCellMar><w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/></w:tblCellMar><w:tblLook w:val=\"0020\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"1\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>",
        line("top", 12),
        line("bottom", 12)
    );
    for _ in &t.columns {
        s += "<w:gridCol w:w=\"1200\"/>";
    }
    s += "</w:tblGrid>";
    // The header row repeats on each page and has the rule beneath it.
    let under = format!("<w:tcBorders>{}</w:tcBorders>", line("bottom", 6));
    s += "<w:tr><w:trPr><w:tblHeader/><w:cantSplit/></w:trPr>";
    for (i, c) in t.columns.iter().enumerate() {
        s += &w_cell(&w_runs(&heading_runs(c), false), i > 0, &under);
    }
    s += "</w:tr>";
    for row in &t.rows {
        s += "<w:tr><w:trPr><w:cantSplit/></w:trPr>";
        for (i, c) in row.iter().enumerate() {
            s += &w_cell(&w_runs(&plain(&cell_text(c)), false), i > 0, "");
        }
        s += "</w:tr>";
    }
    s += "</w:tbl>";
    if !t.footnotes.is_empty() {
        let mut note = vec![Run { text: "Note.".into(), italic: true, sub: false }];
        note.push(Run { text: format!(" {}", t.footnotes.join(" ")), italic: false, sub: false });
        s += &w_para(Some("TableNote"), &w_runs(&note, false));
    } else {
        s += &w_para(Some("TableNote"), "");
    }
    s
}

const W_NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"";

/// A picture in its own paragraph, 6 inches wide, with the alternative text Word and screen readers show.
fn w_picture(rel: &str, id: usize, alt: &str, aspect: f64) -> String {
    let cx: i64 = 5_486_400; // 6 in
    let cy = (cx as f64 * aspect).round() as i64;
    let alt = xml(alt);
    format!("<w:p><w:pPr><w:keepNext/><w:spacing w:after=\"240\"/></w:pPr><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{id}\" name=\"Figure {id}\" descr=\"{alt}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"figure{id}.png\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rel}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>")
}

/// The document body and the PNGs it refers to, in order (rIdImg1, rIdImg2, ...).
fn document_xml(header: &Header, items: &[Item]) -> (String, Vec<Vec<u8>>) {
    let mut body = w_para(Some("Title"), &w_runs(&plain(&header.title), false));
    let mut meta = String::new();
    if let Some(src) = &header.source {
        meta += &format!("Data: {src} · ");
    }
    meta += &format!("Exported {} by PrismStatistics {}", header.date, env!("CARGO_PKG_VERSION"));
    body += &w_para(Some("Meta"), &w_runs(&plain(&meta), false));
    let mut n = 0;
    let mut media: Vec<Vec<u8>> = Vec::new();
    let blocks = |bl: &[Block]| bl.iter().map(|x| if x.heading { w_para(Some("Heading1"), &w_runs(&plain(&x.text), false)) } else { w_para(Some("Results"), &w_runs(&runs(&x.text), false)) }).collect::<String>();
    for it in items {
        let b = it.bundle;
        body += &blocks(&it.before);
        body += &w_para(Some("Heading1"), &w_runs(&plain(&b.title), false));
        body += &w_para(Some("Meta"), &w_runs(&plain(&cases_line(b)), false));
        for (k, c) in b.charts.iter().enumerate().filter(|_| it.parts.charts) {
            let fig = media.len() + 1;
            body += &w_para(Some("TableNumber"), &w_runs(&plain(&format!("Figure {fig}")), false));
            body += &w_para(Some("TableTitle"), &w_runs(&plain(&c.title), false));
            match it.images.get(k).filter(|png| !png.is_empty()) {
                Some(png) => {
                    media.push(png.clone());
                    body += &w_picture(&format!("rIdImg{fig}"), fig, &c.alt, crate::chart::HEIGHT / crate::chart::WIDTH);
                }
                None => {
                    // Keep the numbering; say what the figure would show.
                    media.push(Vec::new());
                    body += &w_para(Some("Meta"), &w_runs(&plain(&format!("[The chart could not be drawn here. {}]", c.alt)), false));
                }
            }
        }
        if it.stale {
            body += &w_para(Some("Warning"), &w_runs(&plain(STALE), false));
        }
        if let Status::Failed { reason } = &b.status {
            body += &w_para(Some("Warning"), &(w_runs(&plain("Not interpreted. "), true) + &w_runs(&plain(reason), false)));
        }
        for t in b.tables.iter().filter(|_| it.parts.tables) {
            n += 1;
            body += &w_table(t, n);
        }
        if it.parts.checks && !b.diagnostics.is_empty() {
            body += &w_para(Some("Heading2"), &w_runs(&plain("Checks"), false));
            for d in &b.diagnostics {
                body += &w_para(Some("Check"), &(w_runs(&plain(&format!("{}: ", d.title)), true) + &w_runs(&plain(&format!("{}. {}", state_words(d.state), d.message)), false)));
            }
        }
        if let Some(p) = it.paragraph.filter(|_| it.parts.paragraph) {
            body += &w_para(Some("Heading2"), &w_runs(&plain(&paragraph_heading(it)), false));
            body += &w_para(Some("Results"), &w_runs(&runs(p), false));
        }
        if it.parts.method {
            body += &w_para(Some("Method"), &(w_runs(&plain("Method. "), true) + &w_runs(&plain(&b.method_note), false)));
        }
    }
    body += &blocks(&header.after);
    // US Letter, one-inch margins.
    body += "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr>";
    (format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document {W_NS}><w:body>{body}</w:body></w:document>"), media)
}

fn styles_xml() -> String {
    let style = |id: &str, name: &str, ppr: &str, rpr: &str| format!("<w:style w:type=\"paragraph\" w:customStyle=\"1\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr>{ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>");
    let heading = |id: &str, name: &str, level: u8, ppr: &str, rpr: &str| format!("<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/>{ppr}<w:outlineLvl w:val=\"{level}\"/></w:pPr><w:rPr>{rpr}</w:rPr></w:style>");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles {W_NS}><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\" w:cs=\"Times New Roman\" w:eastAsia=\"Times New Roman\"/><w:sz w:val=\"24\"/><w:szCs w:val=\"24\"/><w:lang w:val=\"en-US\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"276\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:after=\"60\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"32\"/><w:szCs w:val=\"32\"/></w:rPr></w:style>\
{}{}{}{}{}{}{}{}{}{}{}\
<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"TableNormal\"><w:name w:val=\"Normal Table\"/><w:tblPr><w:tblInd w:w=\"0\" w:type=\"dxa\"/><w:tblCellMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"108\" w:type=\"dxa\"/><w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"108\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style></w:styles>",
        heading("Heading1", "heading 1", 0, "<w:spacing w:before=\"480\" w:after=\"60\"/>", "<w:b/><w:sz w:val=\"28\"/><w:szCs w:val=\"28\"/>"),
        heading("Heading2", "heading 2", 1, "<w:spacing w:before=\"240\" w:after=\"60\"/>", "<w:b/>"),
        style("Meta", "Result Details", "", "<w:color w:val=\"555555\"/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>"),
        style("Warning", "Result Warning", "<w:pBdr><w:left w:val=\"single\" w:sz=\"18\" w:space=\"6\" w:color=\"B45309\"/></w:pBdr>", ""),
        style("TableNumber", "Table Number", "<w:keepNext/><w:spacing w:before=\"240\" w:after=\"0\"/>", "<w:b/>"),
        style("TableTitle", "Table Title", "<w:keepNext/><w:spacing w:after=\"120\"/>", "<w:i/>"),
        style("TableText", "Table Text", "<w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/>", ""),
        style("TableNote", "Table Note", "<w:spacing w:before=\"60\" w:after=\"240\"/>", "<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>"),
        style("Check", "Check", "<w:ind w:left=\"360\"/><w:spacing w:after=\"60\"/>", "<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/>"),
        style("Results", "Results Paragraph", "<w:spacing w:line=\"480\" w:lineRule=\"auto\"/><w:ind w:firstLine=\"720\"/>", ""),
        style("Method", "Method Note", "<w:spacing w:before=\"120\"/>", "<w:color w:val=\"333333\"/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/>"),
    )
}

/// The results as a Word document.
pub fn docx(header: &Header, items: &[Item]) -> Result<Vec<u8>> {
    let (document, media) = document_xml(header, items);
    let image_rels: String = media.iter().enumerate().filter(|(_, m)| !m.is_empty()).map(|(i, _)| format!("<Relationship Id=\"rIdImg{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/figure{}.png\"/>", i + 1, i + 1)).collect();
    let parts: [(&str, String); 6] = [
        (
            "[Content_Types].xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>".into(),
        ),
        (
            "_rels/.rels",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>".into(),
        ),
        (
            "word/_rels/document.xml.rels",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>{image_rels}</Relationships>".replace("{image_rels}", &image_rels),
        ),
        (
            "docProps/core.xml",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title><dc:creator>PrismStatistics</dc:creator></cp:coreProperties>",
                xml(&header.title)
            ),
        ),
        ("word/styles.xml", styles_xml()),
        ("word/document.xml", document),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let err = |e: zip::result::ZipError| Error::Invalid(format!("The document could not be written: {e}"));
    for (name, body) in parts {
        zip.start_file(name, options).map_err(err)?;
        zip.write_all(body.as_bytes())?;
    }
    for (i, png) in media.iter().enumerate().filter(|(_, m)| !m.is_empty()) {
        zip.start_file(format!("word/media/figure{}.png", i + 1), options).map_err(err)?;
        zip.write_all(png)?;
    }
    Ok(zip.finish().map_err(err)?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_becomes_runs() {
        let r = runs("A *t* test, *d*<sub>z</sub> = 1 and 2 * 3");
        let texts: Vec<(&str, bool, bool)> = r.iter().map(|r| (r.text.as_str(), r.italic, r.sub)).collect();
        assert_eq!(texts, [("A ", false, false), ("t", true, false), (" test, ", false, false), ("d", true, false), ("z", false, true), (" = 1 and 2 * 3", false, false)]);
        assert_eq!(heading_runs("p (two-sided)")[0].text, "p");
        assert!(heading_runs("p (two-sided)")[0].italic);
        assert!(!heading_runs("Mean")[0].italic);
        assert_eq!(cell_text(&Cell::P { value: 0.0001 }), "< .001");
        assert_eq!(cell_text(&Cell::Number { value: -1.5, decimals: 2 }), "\u{2212}1.50");
    }
}
