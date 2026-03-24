//! Integration tests for file parsers.
//! These tests read actual fixture files and verify non-empty word arrays.

use speedreading_app_core::core::parser::{detect_and_parse, parse_epub, parse_pdf, parse_txt, tokenize};
use speedreading_app_core::state::{compute_orp_anchor, WordSegment};

fn fixture_path(name: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/{name}")
}

#[test]
fn test_parse_txt_fixture() {
    let path = fixture_path("sample.txt");
    let words = parse_txt(&path).expect("txt parse failed");
    assert!(!words.is_empty(), "txt parser returned empty word list");
    assert!(words.len() >= 10, "expected at least 10 words, got {}", words.len());
}

#[test]
fn test_parse_epub_fixture() {
    let path = fixture_path("sample.epub");
    let words = parse_epub(&path).expect("epub parse failed");
    assert!(!words.is_empty(), "epub parser returned empty word list");
    assert!(words.len() >= 5, "expected at least 5 words, got {}", words.len());
}

#[test]
fn test_parse_pdf_fixture() {
    let path = fixture_path("sample.pdf");
    let words = parse_pdf(&path).expect("pdf parse failed");
    assert!(!words.is_empty(), "pdf parser returned empty word list");
    assert!(words.len() >= 4, "expected at least 4 words, got {}", words.len());
}

#[test]
fn test_detect_and_parse_txt() {
    let path = fixture_path("sample.txt");
    let words = detect_and_parse(&path).expect("detect_and_parse failed for .txt");
    assert!(!words.is_empty());
}

#[test]
fn test_detect_and_parse_unsupported() {
    let result = detect_and_parse("/tmp/test.xyz");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Unsupported"), "expected 'Unsupported' in: {msg}");
}

#[test]
fn test_tokenize_paste_text() {
    // PARSE-01: plain text paste uses the same tokenizer
    let words = tokenize("The quick brown fox");
    assert_eq!(words.len(), 4);
    assert_eq!(words[0], "The");
    assert_eq!(words[3], "fox");
}

// SC-3: ORP anchor pipeline — parse -> compute_orp_anchor per word -> Vec<WordSegment>
// Verifies the full data pipeline for each supported format.

fn check_orp_pipeline(fixture: &str, words: Vec<String>) {
    assert!(!words.is_empty(), "{fixture}: expected non-empty word list");
    let segments: Vec<WordSegment> = words.iter().map(|w| compute_orp_anchor(w)).collect();
    assert_eq!(segments.len(), words.len(), "{fixture}: segment count must equal word count");
    for (i, (word, seg)) in words.iter().zip(segments.iter()).enumerate() {
        let reconstructed = format!("{}{}{}", seg.before, seg.anchor, seg.after);
        assert_eq!(
            reconstructed, *word,
            "{fixture} word[{i}] '{word}': before+anchor+after must equal original word"
        );
        if !word.is_empty() {
            assert_eq!(
                seg.anchor.chars().count(),
                1,
                "{fixture} word[{i}] '{word}': anchor must be exactly 1 char"
            );
        }
    }
}

#[test]
fn test_orp_anchor_pipeline_txt() {
    check_orp_pipeline("sample.txt", parse_txt(&fixture_path("sample.txt")).unwrap());
}

#[test]
fn test_orp_anchor_pipeline_epub() {
    check_orp_pipeline("sample.epub", parse_epub(&fixture_path("sample.epub")).unwrap());
}

#[test]
fn test_orp_anchor_pipeline_pdf() {
    check_orp_pipeline("sample.pdf", parse_pdf(&fixture_path("sample.pdf")).unwrap());
}
