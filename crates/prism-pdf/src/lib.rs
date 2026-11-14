//! Tagged PDF export (spec: tagged PDF in 1.0, decision D09; product owner, 2026-11-11).
//!
//! The report is typeset by Typst (Apache-2.0), embedded as a library, and written as **PDF/UA-1**: a tagged PDF with a
//! structure tree (headings, paragraphs, lists, tables with header rows), the document language and title, and every
//! font embedded. Typst checks PDF/UA-1 conformance while exporting and refuses to write a file that fails it.
//!
//! The layout lives in `report.typ`, a fixed template. The results reach it only as JSON (`data.json`, a file the
//! template reads), never as markup, so labels and values can contain any characters. Fonts come with Typst (Libertinus
//! Serif), so the PDF looks the same on every computer and needs nothing installed.

use std::sync::LazyLock;

use prism_engine::export::{cases_line, cell_text, heading_runs, paragraph_heading, runs, Block, Header, Item, Run, STALE};
use prism_engine::result::Status;
use serde::Serialize;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Smart};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

const TEMPLATE: &str = include_str!("report.typ");

#[derive(Serialize)]
struct Doc {
    title: String,
    subtitle: String,
    results: Vec<ResultOut>,
    after: Vec<BlockOut>,
}

/// A heading or paragraph of the writer's own (Report Builder).
#[derive(Serialize)]
struct BlockOut {
    heading: bool,
    runs: Vec<Run>,
}

fn blocks(bl: &[Block]) -> Vec<BlockOut> {
    bl.iter().map(|b| BlockOut { heading: b.heading, runs: if b.heading { vec![Run { text: b.text.clone(), italic: false, sub: false }] } else { runs(&b.text) } }).collect()
}

#[derive(Serialize)]
struct ResultOut {
    before: Vec<BlockOut>,
    title: String,
    cases: String,
    warnings: Vec<String>,
    tables: Vec<TableOut>,
    figures: Vec<FigureOut>,
    checks: Vec<CheckOut>,
    paragraph: Option<Vec<Run>>,
    paragraph_title: String,
    method: String,
}

#[derive(Serialize)]
struct TableOut {
    number: usize,
    title: String,
    columns: Vec<Vec<Run>>,
    rows: Vec<Vec<String>>,
    note: Option<String>,
}

#[derive(Serialize)]
struct FigureOut {
    number: usize,
    title: String,
    alt: String,
    /// The chart with its text as outlines (Typst has no system fonts).
    svg: String,
}

#[derive(Serialize)]
struct CheckOut {
    title: String,
    state: &'static str,
    message: String,
}

fn document(header: &Header, items: &[Item]) -> Doc {
    let mut subtitle = String::new();
    if let Some(src) = &header.source {
        subtitle += &format!("Data: {src} · ");
    }
    subtitle += &format!("Exported {} by PrismStatistics {}", header.date, env!("CARGO_PKG_VERSION"));
    let mut n = 0;
    let mut fig = 0;
    let results = items
        .iter()
        .map(|it| {
            let b = it.bundle;
            let mut warnings = Vec::new();
            if it.stale {
                warnings.push(STALE.to_string());
            }
            if let Status::Failed { reason } = &b.status {
                warnings.push(format!("Not interpreted. {reason}"));
            }
            let tables = b
                .tables
                .iter()
                .filter(|_| it.parts.tables)
                .map(|t| {
                    n += 1;
                    // Every row is padded to the header's width so the table never shifts.
                    let width = t.columns.len();
                    let rows = t.rows.iter().map(|r| (0..width).map(|i| r.get(i).map(cell_text).unwrap_or_default()).collect()).collect();
                    TableOut { number: n, title: t.title.clone(), columns: t.columns.iter().map(|c| heading_runs(c)).collect(), rows, note: (!t.footnotes.is_empty()).then(|| t.footnotes.join(" ")) }
                })
                .collect();
            let figures = b
                .charts
                .iter()
                .filter(|_| it.parts.charts)
                .filter_map(|c| {
                    fig += 1;
                    Some(FigureOut { number: fig, title: c.title.clone(), alt: c.alt.clone(), svg: outline_svg(&c.svg)? })
                })
                .collect();
            ResultOut {
                before: blocks(&it.before),
                title: b.title.clone(),
                cases: cases_line(b),
                warnings,
                tables,
                figures,
                checks: b.diagnostics.iter().filter(|_| it.parts.checks).map(|d| CheckOut { title: d.title.clone(), state: d.state.words(), message: d.message.clone() }).collect(),
                paragraph: it.paragraph.filter(|_| it.parts.paragraph).map(runs),
                paragraph_title: paragraph_heading(it),
                method: if it.parts.method { b.method_note.clone() } else { String::new() },
            }
        })
        .collect();
    Doc { title: header.title.clone(), subtitle, results, after: blocks(&header.after) }
}

static FONTS: LazyLock<(LazyHash<FontBook>, Vec<Font>)> = LazyLock::new(|| {
    let fonts: Vec<Font> = typst_assets::fonts().flat_map(|d| Font::iter(Bytes::new(d))).collect();
    (LazyHash::new(FontBook::from_fonts(&fonts)), fonts)
});

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

/// What Typst may see: the template, the data, the bundled fonts. No files from disk, no packages, no network.
struct Report {
    main: Source,
    data_id: FileId,
    data: Bytes,
}

fn file_id(name: &str) -> FileId {
    FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new(name).expect("a plain file name")))
}

impl World for Report {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.0
    }
    fn main(&self) -> FileId {
        self.main.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.data_id {
            Ok(self.data.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn font(&self, index: usize) -> Option<Font> {
        FONTS.1.get(index).cloned()
    }
    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        None
    }
}

/// The template and the data it reads, for tools that render the same report another way (such as page previews).
pub fn template_and_data(header: &Header, items: &[Item]) -> (&'static str, Vec<u8>) {
    (TEMPLATE, serde_json::to_vec(&document(header, items)).expect("the report data always serializes"))
}

/// The results as a tagged PDF/UA-1 file.
pub fn pdf(header: &Header, items: &[Item]) -> Result<Vec<u8>, String> {
    let data = serde_json::to_vec(&document(header, items)).map_err(|e| format!("The report could not be prepared: {e}"))?;
    let world = Report { main: Source::new(file_id("report.typ"), TEMPLATE.into()), data_id: file_id("data.json"), data: Bytes::new(data) };
    let messages = |d: &[typst::diag::SourceDiagnostic]| d.iter().map(|d| d.message.to_string()).collect::<Vec<_>>().join("; ");
    let doc = typst::compile::<typst_layout::PagedDocument>(&world).output.map_err(|e| format!("The PDF could not be laid out: {}", messages(&e)))?;
    let options = typst_pdf::PdfOptions {
        ident: Smart::Auto,
        creator: Smart::Custom(Some(format!("PrismStatistics {}", env!("CARGO_PKG_VERSION")))),
        timestamp: None,
        page_ranges: None,
        standards: typst_pdf::PdfStandards::new(&[typst_pdf::PdfStandard::Ua_1]).map_err(|e| format!("The PDF settings are invalid: {e:?}"))?,
        tagged: true,
        pretty: false,
    };
    typst_pdf::pdf(&doc, &options).map_err(|e| format!("The PDF could not be written as accessible PDF/UA-1: {}", messages(&e)))
}

// ---------------------------------------------------------------- charts

/// Fonts for chart text: the computer's own (Arial on Windows), with Typst's bundled fonts as a fallback so a chart
/// always has text even where no system font matches.
static CHART_FONTS: LazyLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = LazyLock::new(|| {
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    for data in typst_assets::fonts() {
        db.load_font_data(data.to_vec());
    }
    if db.faces().all(|f| f.families.iter().all(|(n, _)| n != "Arial")) {
        db.set_sans_serif_family("Libertinus Serif");
    }
    std::sync::Arc::new(db)
});

/// The largest chart drawn, in pixels on a side before scaling. The engine's charts are well under it; a chart from a
/// damaged or edited project that declares a bigger size is refused rather than given gigabytes of memory.
const MAX_SIDE: f32 = 4000.0;

fn chart_tree(svg: &str) -> Option<resvg::usvg::Tree> {
    // Images may come only from data inside the chart itself, never from a file or a network path.
    let images = resvg::usvg::ImageHrefResolver { resolve_data: resvg::usvg::ImageHrefResolver::default_data_resolver(), resolve_string: Box::new(|_, _| None) };
    let opt = resvg::usvg::Options { fontdb: CHART_FONTS.clone(), image_href_resolver: images, resources_dir: None, ..Default::default() };
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let s = tree.size();
    (s.width() <= MAX_SIDE && s.height() <= MAX_SIDE).then_some(tree)
}

/// The chart with its text turned into outlines, so it looks the same wherever it is placed (Typst sees no fonts).
pub fn outline_svg(svg: &str) -> Option<String> {
    Some(chart_tree(svg)?.to_string(&resvg::usvg::WriteOptions::default()))
}

/// The chart as a PNG at `scale` times its size (2 gives a sharp image in Word at the usual 6-inch width).
pub fn chart_png(svg: &str, scale: f32) -> Option<Vec<u8>> {
    let tree = chart_tree(svg)?;
    let scale = scale.clamp(0.1, 4.0);
    let size = tree.size().to_int_size().scale_by(scale)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_engine::result::{CaseAccounting, Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

    fn bundle() -> ResultBundle {
        ResultBundle {
            method_id: "test".into(),
            method_version: 1,
            title: "Crosstabs: \"Group\" [A] by #tag & $cost \\ *x*".into(),
            settings: serde_json::Value::Null,
            status: Status::Ok,
            cases: CaseAccounting { total: 10, analyzed: 9, excluded: vec![] },
            tables: vec![Table {
                id: "t".into(),
                title: "Group statistics".into(),
                columns: vec!["Group".into(), "N".into(), "p (two-sided)".into()],
                rows: vec![vec![Cell::Text { text: "Urban = #1 \"quoted\"".into() }, Cell::Count { value: 4 }, Cell::P { value: 0.0004 }], vec![Cell::Text { text: "short row".into() }]],
                footnotes: vec!["Mean difference = Urban minus Rural.".into()],
            }],
            diagnostics: vec![Diagnostic { id: "s".into(), title: "Sample size".into(), state: DiagnosticState::ReviewNeeded, message: "Small.".into() }],
            method_note: "Welch's t test.".into(),
            fields: Default::default(),
            charts: Vec::new(),
        }
    }

    #[test]
    fn writes_a_tagged_pdf_ua_file_whatever_the_text() {
        let b = bundle();
        let items = [Item { bundle: &b, paragraph: Some("*t*(9) = 2.10, *p* = .049, *d*<sub>z</sub> = 0.45"), stale: true, ai_model: None, images: Vec::new(), parts: Default::default(), before: Vec::new() }];
        let header = Header { title: "study: Output".into(), source: Some("study.csv".into()), date: "November 11, 2026".into(), after: Vec::new() };
        let bytes = pdf(&header, &items).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let raw = String::from_utf8_lossy(&bytes);
        for marker in ["/StructTreeRoot", "/MarkInfo", "/Lang", "pdfuaid:part"] {
            assert!(raw.contains(marker), "missing {marker}");
        }
    }
}

#[cfg(test)]
mod untrusted_svg_tests {
    /// A picture naming a file on disk draws without it: images come only from data inside the picture.
    #[test]
    fn outside_images_are_never_read_and_huge_sizes_are_refused() {
        let dir = std::env::temp_dir().join(format!("prism-svg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let red = dir.join("red.png");
        let mut px = resvg::tiny_skia::Pixmap::new(10, 10).unwrap();
        px.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        std::fs::write(&red, px.encode_png().unwrap()).unwrap();
        let with = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="{}" width="10" height="10"/></svg>"#, red.display());
        let without = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"></svg>"#;
        assert_eq!(super::chart_png(&with, 1.0), super::chart_png(without, 1.0), "the file was not drawn");
        assert!(super::chart_png(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"/>"#, 2.0).is_none());
        assert!(super::chart_png(without, 1e9).is_some(), "the scale is clamped");
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod diagram_tests {
    /// A path diagram (markers at both ends, curves, ellipses, a hexagon) draws for Word and PDF.
    #[test]
    fn path_diagrams_render() {
        use prism_engine::analysis::sem::diagram;
        let mut d = diagram::structure("f =~ x1 + x2\ng <~ x3 + x4\ny ~ f + g\nx1 ~~ x2").unwrap();
        diagram::layout(&mut d, &Default::default());
        let svg = diagram::svg(&d, &std::collections::BTreeMap::from([("f =~ x1".to_string(), ".80".to_string())]), "A path diagram.");
        let png = super::chart_png(&svg, 2.0).expect("the diagram renders");
        assert!(png.len() > 1000);
        if let Ok(dir) = std::env::var("PRISM_DIAGRAM_DIR") {
            std::fs::write(format!("{dir}/diagram.png"), &png).unwrap();
        }
    }
}
