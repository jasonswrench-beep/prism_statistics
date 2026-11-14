//! Chart pictures read from a project file are rebuilt before anything shows or exports them. A project is a zip anyone
//! can edit, so its stored SVG is untrusted: it could hold scripts, links, embedded HTML (`foreignObject`) or images
//! that point at files or network shares. The picture is parsed and written out again keeping only the elements and
//! attributes the engine's own charts and diagrams use (chart.rs, analysis/sem/diagram.rs), with plain values. Anything
//! else is dropped with everything inside it, and a picture that can't be read, or declares a huge size, is dropped.

use roxmltree::{Document, Node, NodeType, ParsingOptions};

const ELEMENTS: &[&str] = &["svg", "g", "rect", "line", "polyline", "polygon", "path", "circle", "ellipse", "text", "tspan", "title", "desc", "defs", "marker"];

const ATTRIBUTES: &[&str] = &[
    "viewBox", "width", "height", "role", "aria-label", "font-family", "font-size", "font-weight", "font-style", "fill", "fill-opacity", "stroke", "stroke-width", "stroke-dasharray", "stroke-opacity", "stroke-linecap", "stroke-linejoin", "opacity", "x", "y",
    "x1", "y1", "x2", "y2", "dx", "dy", "cx", "cy", "r", "rx", "ry", "points", "d", "text-anchor", "dominant-baseline", "paint-order", "transform", "id", "refX", "refY", "markerWidth", "markerHeight", "orient", "marker-end", "marker-start", "data-param",
];

/// The largest size a chart may declare, in pixels on a side.
const MAX_SIDE: f64 = 4000.0;
/// The longest stored picture read at all.
const MAX_LEN: usize = 4 << 20;

fn escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
}

/// Whether an attribute's value is plain: free text only where it is text (labels), and otherwise numbers, colors,
/// words and path data. The only reference allowed is to a marker defined in the same picture: `url(#name)`.
fn value_ok(name: &str, v: &str) -> bool {
    match name {
        "aria-label" | "data-param" | "font-family" => !v.to_ascii_lowercase().contains("url("),
        "marker-end" | "marker-start" => v.strip_prefix("url(#").and_then(|r| r.strip_suffix(')')).is_some_and(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')),
        "id" => v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        _ => v.chars().all(|c| c.is_ascii_alphanumeric() || " #.,-+()%".contains(c)) && !v.to_ascii_lowercase().contains("url"),
    }
}

fn size_ok(v: &str) -> bool {
    v.split([' ', ',']).filter(|t| !t.is_empty()).all(|t| t.trim_end_matches("px").parse::<f64>().is_ok_and(|n| n.is_finite() && n.abs() <= MAX_SIDE * 4.0))
}

fn write(node: Node, out: &mut String) -> bool {
    match node.node_type() {
        NodeType::Text => {
            escape(node.text().unwrap_or(""), out);
            true
        }
        NodeType::Element => {
            let name = node.tag_name().name();
            if !ELEMENTS.contains(&name) || node.tag_name().namespace().is_some_and(|ns| ns != "http://www.w3.org/2000/svg") {
                return true; // dropped, with everything inside it
            }
            out.push('<');
            out.push_str(name);
            if name == "svg" {
                out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
            }
            for a in node.attributes() {
                if a.namespace().is_some() || !ATTRIBUTES.contains(&a.name()) || !value_ok(a.name(), a.value()) {
                    continue;
                }
                if name == "svg" && matches!(a.name(), "width" | "height" | "viewBox") && !size_ok(a.value()) {
                    return false;
                }
                if name == "svg" && matches!(a.name(), "width" | "height") && a.value().trim_end_matches("px").parse::<f64>().is_ok_and(|n| n > MAX_SIDE) {
                    return false;
                }
                out.push(' ');
                out.push_str(a.name());
                out.push_str("=\"");
                escape(a.value(), out);
                out.push('"');
            }
            out.push('>');
            for c in node.children() {
                if !write(c, out) {
                    return false;
                }
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
            true
        }
        _ => true, // comments and processing instructions
    }
}

/// The picture rebuilt from its allowed parts, or None when it isn't an SVG picture, can't be read, or is too big.
pub fn clean(svg: &str) -> Option<String> {
    if svg.len() > MAX_LEN {
        return None;
    }
    // No DTD: no entities, so nothing can expand or point outside.
    let doc = Document::parse_with_options(svg, ParsingOptions { allow_dtd: false, ..Default::default() }).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return None;
    }
    let mut out = String::with_capacity(svg.len());
    write(root, &mut out).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn the_engines_own_pictures_come_through_whole() {
        let ds = {
            let mut d = crate::Dataset::new("t");
            d.add_variable("y", crate::Column::Numeric((0..20).map(|i| Some(f64::from(i % 7))).collect())).unwrap();
            d
        };
        let out = crate::analysis::graphs::run(&ds, &serde_json::from_value(serde_json::json!({ "kind": "histogram", "variables": ["y"] })).unwrap()).unwrap();
        let svg = &out[0].charts[0].svg;
        let c = clean(svg).unwrap();
        // Same elements and text; only attribute quoting and the namespace may be rewritten.
        let count = |s: &str, t: &str| s.matches(t).count();
        for t in ["<rect", "<line", "<text", "<title>", "role=\"img\"", "aria-label="] {
            assert_eq!(count(svg, t), count(&c, t), "{t}");
        }
        assert_eq!(clean(&c).unwrap(), c, "cleaning is stable");
    }

    #[test]
    fn scripts_links_html_and_outside_images_are_dropped() {
        let evil = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100" height="100" onload="alert(1)">
            <script>alert(1)</script>
            <foreignObject><div xmlns="http://www.w3.org/1999/xhtml"><a href="https://x.example">Click</a></div></foreignObject>
            <image href="\\host\x.png" width="10" height="10"/><image xlink:href="file:///C:/secret.png"/>
            <a href="javascript:alert(1)"><text x="1" y="2">fake</text></a>
            <use href="#x"/><style>@import url(https://x.example/s.css);</style>
            <rect width="10" height="10" fill="url(https://x.example/p)" style="fill:red" onclick="x()"/>
            <text x="5" y="5">Mean &lt; 3</text>
            <path d="M0,0 L10,5" marker-end="url(#arrow)"/><path d="M0,0" marker-end="url(https://x)"/>
        </svg>"##;
        let c = clean(evil).unwrap();
        for bad in ["script", "alert", "foreignObject", "<div", "href", "image", "javascript", "<a", "<use", "style", "onclick", "onload", "https", "file:", "host"] {
            assert!(!c.contains(bad), "{bad} in {c}");
        }
        assert!(c.contains("<text x=\"5\" y=\"5\">Mean &lt; 3</text>") && c.contains("marker-end=\"url(#arrow)\""), "{c}");
        assert!(c.contains("<rect width=\"10\" height=\"10\">"), "the bad fill goes, the rectangle stays: {c}");
    }

    #[test]
    fn huge_or_unreadable_pictures_are_refused() {
        assert!(clean(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"/>"#).is_none());
        assert!(clean(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100000 100000"/>"#).is_none());
        assert!(clean(r#"<!DOCTYPE svg [<!ENTITY a "aaaa">]><svg xmlns="http://www.w3.org/2000/svg">&a;</svg>"#).is_none());
        assert!(clean("<html><body/></html>").is_none());
        assert!(clean("not xml").is_none());
    }
}
