//! File parsers for all supported input types.
//! All parsers return Vec<String> — the actor thread calls compute_orp_anchor per word.

use anyhow::Context;

/// Tokenize text by splitting on whitespace, filtering empty tokens.
/// Used by all parsers and by LoadText (paste) action.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Parse a plain text file at the given path.
pub fn parse_txt(path: &str) -> anyhow::Result<Vec<String>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read file: {path}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let words = tokenize(&text);
    if words.is_empty() {
        anyhow::bail!("The text file appears to be empty.");
    }
    Ok(words)
}

/// Parse an EPUB file at the given path.
/// Iterates the spine (not TOC) for EPUB 2 + 3 compatibility.
pub fn parse_epub(path: &str) -> anyhow::Result<Vec<String>> {
    use epub::doc::EpubDoc;

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open EPUB: {path}"))?;
    let reader = std::io::BufReader::new(file);
    let mut doc = EpubDoc::from_reader(reader)
        .map_err(|e| anyhow::anyhow!("EPUB parse error: {e}"))?;

    let mut full_text = String::new();

    // Read first spine item
    if let Some((content, _mime)) = doc.get_current_str() {
        full_text.push_str(&strip_html(&content));
    }
    // Iterate remaining spine items
    while doc.go_next() {
        if let Some((content, _mime)) = doc.get_current_str() {
            full_text.push(' ');
            full_text.push_str(&strip_html(&content));
        }
    }

    let words = tokenize(&full_text);
    if words.is_empty() {
        anyhow::bail!("EPUB appears to be empty or could not be read.");
    }
    Ok(words)
}

/// Parse a PDF file at the given path.
/// Uses pdf-extract (not lopdf) for CID font support.
/// Returns Err with user-facing message for image-only PDFs.
pub fn parse_pdf(path: &str) -> anyhow::Result<Vec<String>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("Failed to read PDF: {path}"))?;

    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| anyhow::anyhow!("PDF parse error: {e}"))?;

    let words = tokenize(&text);
    if words.is_empty() {
        anyhow::bail!(
            "This PDF contains only images — no text layer found. \
             Try a PDF with selectable text, or paste the text directly."
        );
    }
    Ok(words)
}

/// Detect file format from extension and dispatch to the correct parser.
pub fn detect_and_parse(path: &str) -> anyhow::Result<Vec<String>> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" => parse_txt(path),
        "epub" => parse_epub(path),
        "pdf" => parse_pdf(path),
        other => anyhow::bail!("Unsupported file format: .{other}"),
    }
}

/// Strip HTML tags using a simple char-by-char state machine.
/// Removes <head>, <script>, <style> block content to avoid garbage words.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_skip_block = false;
    let mut tag_buf = String::new();

    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag_buf.clear();
            }
            '>' => {
                let tag_lower = tag_buf.trim().to_lowercase();
                // Start skip blocks
                if tag_lower.starts_with("head")
                    || tag_lower.starts_with("script")
                    || tag_lower.starts_with("style")
                {
                    in_skip_block = true;
                }
                // End skip blocks
                if tag_lower.starts_with("/head")
                    || tag_lower.starts_with("/script")
                    || tag_lower.starts_with("/style")
                {
                    in_skip_block = false;
                }
                in_tag = false;
                tag_buf.clear();
                if !in_skip_block {
                    out.push(' '); // replace tag with space for word separation
                }
            }
            _ if in_tag => {
                tag_buf.push(c);
            }
            _ if !in_skip_block => {
                out.push(c);
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let words = tokenize("hello world  foo");
        assert_eq!(words, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn test_tokenize_empty() {
        let words = tokenize("");
        assert!(words.is_empty());
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        let words = tokenize("   \n\t  ");
        assert!(words.is_empty());
    }
}
