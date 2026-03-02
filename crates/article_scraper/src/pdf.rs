use mupdf::{MetadataName, TextPageOptions};
use result::ArticlerResult;
use std::path::Path;
use url::Url;

use crate::{ArticleMimeType, Document, helpers::reading_time};

pub struct PdfExtractor {}

// This file here and there use log:error than skip pattern. It is not good approach, but concretly for this code it is ok, because extractors polluted by different parsing errors, which is not interesting for any code outside. Extractors must just provide the suitable fallback document in these cases.

impl PdfExtractor {
    pub fn extract(url: &Url, data: &[u8]) -> Document {
        let doc = match mupdf::Document::from_bytes(data, "application/pdf") {
            Ok(doc) => Some(doc),
            Err(e) => {
                log::warn!("Failed to parse PDF from {url}: {e:?}");
                None
            }
        };

        let Some(doc) = doc else {
            let title = get_file_name(url)
                .and_then(|f| {
                    std::path::Path::new(&f.to_lowercase())
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_default();

            return Document {
                title,
                content_html: String::new(),
                content_text: String::new(),
                image_url: None,
                mime_type: Some(ArticleMimeType::Pdf.to_string()),
                language: None,
                published_at: None,
                reading_time: 0,
            };
        };

        let content_text = match extract_raw_text(&doc) {
            Ok(Some(t)) => t,
            Err(e) => {
                log::warn!("Pdf text extractions failed on url {url:?} with error: {e}");
                String::new()
            }
            _ => String::new(),
        };

        let reading_time = match reading_time(&content_text) {
            Ok(r) => r,
            Err(_e) => {
                log::error!("Can't calculate reading time for pdf article {url:?}");
                0
            }
        };

        let title = get_pdf_title(url, &doc)
            .or_else(|| {
                get_file_name(url).and_then(|f| {
                    std::path::Path::new(&f.to_lowercase())
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(ToOwned::to_owned)
                })
            })
            .unwrap_or_default();

        Document {
            title,
            content_html: String::new(),
            content_text,
            image_url: None,
            mime_type: Some(ArticleMimeType::Pdf.to_string()),
            language: None,
            published_at: None,
            reading_time,
        }
    }
}

fn get_file_name(url: &Url) -> Option<&str> {
    if let Some(mut segments) = url.path_segments()
        && let Some(last) = segments.next_back()
        && !last.is_empty()
    {
        Some(last)
    } else {
        None
    }
}

fn get_pdf_title(url: &Url, doc: &mupdf::Document) -> Option<String> {
    let title = match extract_title_from_metadata(doc) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("Extract pdf title from metadata error: {e:?} with url {url}");
            None
        }
    };

    let filename = get_file_name(url).map(str::to_lowercase);
    let filename = filename.as_deref().and_then(|f| Path::new(f).file_stem());

    if let Some(title) = title {
        let title_base = title
            .rsplit_once('.')
            .map_or(title.as_str(), |(base, _)| base)
            .to_lowercase();

        let Some(filename) = filename else {
            return Some(title);
        };

        // Check if title from metadata is not just filename - some pdfs do it
        if title_base.as_str() != filename {
            return Some(title);
        }
    }

    match extract_title_from_content(doc) {
        Ok(t) if t.is_some() => return t,
        Err(e) => {
            log::warn!("Extract pdf title from content error: {e:?} with url {url}");
        }
        _ => {}
    }

    // Fallback to filename
    filename.and_then(|f| f.to_str().map(ToOwned::to_owned))
}

fn extract_title_from_metadata(doc: &mupdf::Document) -> ArticlerResult<Option<String>> {
    let title = doc.metadata(MetadataName::Title)?;

    if title.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(title))
    }
}

fn extract_title_from_content(doc: &mupdf::Document) -> ArticlerResult<Option<String>> {
    let first_page = doc.load_page(0)?;

    let text_page = first_page.to_text_page(TextPageOptions::empty())?;

    let mut blocks: Vec<(String, f32)> = Vec::new();
    let mut max_font_size: f32 = 0.0;

    // Scan all text blocks to find max font size
    for block in text_page.blocks() {
        let block_text = extract_block_text(&block);
        let block_text_trimmed = block_text.trim();
        if block_text_trimmed.is_empty() {
            continue;
        }

        let font_size = get_block_font_size(&block);

        if font_size > 0.0 {
            blocks.push((block_text, font_size));
        }

        max_font_size = max_font_size.max(font_size);
    }

    if blocks.is_empty() {
        return Ok(None);
    }

    // Get blocks with max font size
    let title_blocks: Vec<_> = blocks
        .into_iter()
        .filter(|(_, f)| (f - max_font_size).abs() < 0.1)
        .collect();

    if title_blocks.is_empty() {
        return Ok(None);
    }

    // Join all title blocks
    let title: String = title_blocks
        .iter()
        .map(|(text, _)| text.trim())
        .collect::<Vec<_>>()
        .join(" ");

    if looks_like_valid_title(&title) {
        Ok(Some(title))
    } else {
        Ok(None)
    }
}

fn extract_block_text(block: &mupdf::text_page::TextBlock) -> String {
    let mut text = String::new();
    for line in block.lines() {
        for ch in line.chars() {
            if let Some(c) = ch.char() {
                text.push(c);
            }
        }
        text.push(' ');
    }
    text
}

fn get_block_font_size(block: &mupdf::text_page::TextBlock) -> f32 {
    let mut max_size = 0.0_f32;

    for line in block.lines() {
        for ch in line.chars() {
            let size = ch.size();
            if size > max_size {
                max_size = size;
            }
        }
    }

    max_size
}

fn looks_like_valid_title(text: &str) -> bool {
    let text = text.trim();

    let char_count = text.chars().count();

    if char_count < 10 {
        return false;
    }

    let spaces_count = text.chars().filter(|&c| c == ' ').count();
    if spaces_count > char_count / 3 {
        return false;
    }

    let non_printable = text
        .chars()
        .filter(|c| !c.is_ascii_graphic() && !c.is_whitespace())
        .count();
    if non_printable > char_count / 10 {
        return false;
    }

    true
}

fn extract_raw_text(doc: &mupdf::Document) -> ArticlerResult<Option<String>> {
    let pages = doc.pages()?;

    let mut text = String::new();

    for page in pages.flatten() {
        let Ok(text_page) = page.to_text_page(TextPageOptions::empty()) else {
            continue;
        };

        for block in text_page.blocks() {
            let block_text = extract_block_text(&block);
            text.push_str(&block_text);
            text.push('\n');
        }
    }

    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use crate::pdf::extract_title_from_metadata;

    use super::PdfExtractor;

    fn extract_title(filename: &str, data: &[u8]) -> String {
        let url = Url::parse(&format!("http://example.com/{filename}")).unwrap();
        PdfExtractor::extract(&url, data).title
    }

    #[test]
    fn test_title_2310_11703v2() {
        let title = extract_title(
            "2310.11703v2.pdf",
            include_bytes!("../test_articles/2310.11703v2.pdf"),
        );
        assert_eq!(
            "A Comprehensive Survey on Vector Database: Storage and Retrieval Technique, Challenge",
            title
        );
    }

    #[test]
    fn test_title_2412_02792v1() {
        let title = extract_title(
            "2412.02792v1.pdf",
            include_bytes!("../test_articles/2412.02792v1.pdf"),
        );
        assert_eq!(
            "Taurus Database: How to be Fast, Available, and Frugal in the Cloud",
            title
        );
    }

    #[test]
    fn test_title_inter_process_communication() {
        let title = extract_title(
            "inter-process_communication_in_linux.pdf",
            include_bytes!("../test_articles/inter-process_communication_in_linux.pdf"),
        );
        assert_eq!("A guide to inter-process  communication in Linux", title);
    }

    #[test]
    fn test_title_p2115_leis() {
        let title = extract_title(
            "p2115-leis.pdf",
            include_bytes!("../test_articles/p2115-leis.pdf"),
        );
        assert_eq!(
            "Cloud-Native Database Systems and Unikernels: Reimagining OS Abstractions for Modern Hardware",
            title
        );
    }

    #[test]
    fn test_title_p48_neumann() {
        let title = extract_title(
            "p48-neumann.pdf",
            include_bytes!("../test_articles/p48-neumann.pdf"),
        );
        assert_eq!(
            "A Critique of Modern SQL And A Proposal Towards A Simple and Expressive Query Language",
            title
        );
    }

    #[test]
    fn test_title_sosp87_timing_wheels() {
        let title = extract_title(
            "sosp87-timing-wheels.pdf",
            include_bytes!("../test_articles/sosp87-timing-wheels.pdf"),
        );
        assert_eq!(
            "Hashed and Hierarchical Timing Wheels: Data Structures for the Efficient Implementation of a Timer Facility",
            title
        );
    }

    #[test]
    fn test_title_vmcai20b() {
        let title = extract_title(
            "vmcai20b.pdf",
            include_bytes!("../test_articles/vmcai20b.pdf"),
        );
        assert_eq!("The Siren Song of Temporal Synthesis", title);
    }

    #[test]
    fn test_title_260108109v1() {
        let title = extract_title(
            "2601.08109v1.pdf",
            include_bytes!("../test_articles/2601.08109v1.pdf"),
        );
        assert_eq!("CSQL: Mapping Documents into Causal Databases", title);
    }

    #[test]
    fn test_metadata_title_260108109v1() {
        let doc = mupdf::Document::from_bytes(
            include_bytes!("../test_articles/2601.08109v1.pdf"),
            "application/pdf",
        )
        .unwrap();

        let title = extract_title_from_metadata(&doc).unwrap().unwrap();

        assert_eq!("CSQL: Mapping Documents into Causal Databases", title);
    }
}
