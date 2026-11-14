//! Opening a Google Sheets spreadsheet from its link (product owner decision, 2026-11-11: shared link, no sign-in).
//!
//! The sheet must be shared as "Anyone with the link can view". PrismStatistics asks Google for that spreadsheet as an
//! .xlsx file, the same download a person gets from File › Download in the browser, and then reads it like any other
//! workbook (so a file with several tabs asks which one). Nothing is sent to Google except the request for the sheet:
//! no account, no sign-in, no data. Private sheets can't be opened this way, which is deliberate.

use std::io::Read;

/// Downloads stop here; a teaching or research sheet is far smaller.
const MAX_BYTES: u64 = 100 * 1024 * 1024;

/// The spreadsheet ID in a Google Sheets link, or None when the text isn't one.
pub fn sheet_id(link: &str) -> Option<String> {
    let link = link.trim();
    let rest = link.split("docs.google.com/spreadsheets/").nth(1)?;
    // Links look like .../spreadsheets/d/<id>/edit#gid=0, or .../spreadsheets/u/1/d/<id>/...
    let mut parts = rest.split('/');
    while let Some(p) = parts.next() {
        if p == "d" {
            let id = parts.next()?;
            let id: String = id.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
            return (id.len() >= 20).then_some(id);
        }
    }
    None
}

/// The file name Google gives the download, from its Content-Disposition header.
fn file_name(disposition: &str) -> Option<String> {
    if let Some(enc) = disposition.split("filename*=UTF-8''").nth(1) {
        let raw = enc.split(';').next()?.trim();
        return percent_decode(raw);
    }
    let quoted = disposition.split("filename=\"").nth(1)?;
    Some(quoted.split('"').next()?.to_string())
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Downloads the spreadsheet at `link` as .xlsx. Returns the file name (the sheet's title) and the bytes.
pub fn download(link: &str) -> Result<(String, Vec<u8>), String> {
    let id = sheet_id(link).ok_or("That doesn't look like a shared spreadsheet link. It should start with https://docs.google.com/spreadsheets/d/")?;
    let url = format!("https://docs.google.com/spreadsheets/d/{id}/export?format=xlsx");
    let not_shared = "PrismStatistics couldn't open this sheet. In the spreadsheet, choose Share › General access › \"Anyone with the link\" (Viewer), then try again. Private sheets can't be opened, so no sign-in is ever needed.";
    let agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(60))).user_agent(concat!("PrismStatistics/", env!("CARGO_PKG_VERSION"))).build().new_agent();
    let mut response = match agent.get(&url).call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => return Err("There is no spreadsheet at that link. Check that it was copied in full.".into()),
        Err(ureq::Error::StatusCode(401 | 403)) => return Err(not_shared.into()),
        Err(ureq::Error::StatusCode(code)) => return Err(format!("The spreadsheet service answered with an error ({code}). Try again in a moment.")),
        Err(e) => return Err(format!("PrismStatistics couldn't reach the spreadsheet service. Check the internet connection and try again. ({e})")),
    };
    let disposition = response.headers().get("content-disposition").and_then(|v| v.to_str().ok()).map(str::to_string);
    let mut bytes = Vec::new();
    response.body_mut().as_reader().take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("The download of the spreadsheet was interrupted: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("This spreadsheet is larger than 100 MB, which is too large to download. Download it as .xlsx or .csv yourself and open the file.".into());
    }
    // A sheet that isn't shared comes back as a sign-in page rather than a workbook.
    if !bytes.starts_with(b"PK") {
        return Err(not_shared.into());
    }
    let name = disposition.as_deref().and_then(file_name).filter(|n| n.to_ascii_lowercase().ends_with(".xlsx")).unwrap_or_else(|| "Google Sheet.xlsx".into());
    Ok((name, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_id_in_the_usual_link_shapes() {
        let id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        for link in [
            format!("https://docs.google.com/spreadsheets/d/{id}/edit#gid=0"),
            format!("https://docs.google.com/spreadsheets/d/{id}/edit?usp=sharing"),
            format!("  https://docs.google.com/spreadsheets/u/1/d/{id}/view  "),
            format!("docs.google.com/spreadsheets/d/{id}"),
        ] {
            assert_eq!(sheet_id(&link).as_deref(), Some(id), "{link}");
        }
        assert_eq!(sheet_id("https://example.com/spreadsheets/d/abc"), None);
        assert_eq!(sheet_id("https://docs.google.com/spreadsheets/d/short/edit"), None);
        assert_eq!(sheet_id("not a link"), None);
    }

    #[test]
    fn reads_the_title_from_the_download_header() {
        assert_eq!(file_name("attachment; filename=\"Class Data.xlsx\"; filename*=UTF-8''Class%20Data.xlsx").as_deref(), Some("Class Data.xlsx"));
        assert_eq!(file_name("attachment; filename=\"Survey.xlsx\"").as_deref(), Some("Survey.xlsx"));
        assert_eq!(file_name("attachment; filename*=UTF-8''Encuesta%20%C3%B1.xlsx").as_deref(), Some("Encuesta ñ.xlsx"));
    }
}
