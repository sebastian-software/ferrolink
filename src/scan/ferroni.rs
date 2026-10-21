//! The Ferroni backend: regular expressions over the raw text, no tokenizer.
//!
//! One pattern finds comments, raw-text elements with their content, start
//! tags, and end tags, so that links inside comments and scripts are skipped.
//! A second pattern splits a start tag's attributes. This is the
//! "we only need the links" approach; where it differs from a WHATWG tokenizer
//! is what the backend comparison tests measure.

use std::sync::LazyLock;

use ferroni::prelude::Regex;

use super::{Collector, Element, PageScan, decode_char_refs};

/// Groups: 1 raw-text name, 2 its attributes; 3 start tag name, 4 its
/// attributes; 5 end tag name. Comments match without a group.
const MARKUP: &str = concat!(
    r#"<!--[\s\S]*?(?:-->|\z)"#,
    r#"|<((?i:script|style|textarea|title|xmp|iframe|noembed|noframes|noscript))\b"#,
    r#"((?:[^>"']++|"[^"]*+"|'[^']*+')*+)>[\s\S]*?(?:</(?i:\1)\s*>|\z)"#,
    r#"|<([a-zA-Z][^\s/>]*+)((?:[^>"']++|"[^"]*+"|'[^']*+')*+)>"#,
    r#"|</([a-zA-Z][^\s/>]*+)[^>]*+>"#,
);

/// Groups: 1 name; value in 2 (double-quoted), 3 (single-quoted), or 4 (bare).
const ATTRIBUTE: &str =
    r#"([^\s"'>/=][^\s"'>/=]*+)(?:\s*+=\s*+(?:"([^"]*+)"|'([^']*+)'|([^\s>]++)))?"#;

static MARKUP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(MARKUP).expect("the markup pattern is a valid Oniguruma regex"));
static ATTRIBUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ATTRIBUTE).expect("the attribute pattern is a valid Oniguruma regex")
});

pub(super) fn scan(html: &[u8]) -> PageScan {
    let text = String::from_utf8_lossy(html);
    let mut collector = Collector::default();
    let mut name = Vec::new();
    for captures in MARKUP_RE.captures_iter(&text) {
        let start = captures.get(0).map_or(0, |whole| whole.start());
        if let Some(end_name) = captures.get(5) {
            lowercase_into(&mut name, end_name.as_bytes());
            collector.end_tag(&name);
            continue;
        }
        let Some((tag, attributes)) = captures
            .get(1)
            .zip(captures.get(2))
            .or_else(|| captures.get(3).zip(captures.get(4)))
        else {
            continue;
        };
        lowercase_into(&mut name, tag.as_bytes());
        collector.open_tag(&name, start);
        let attributes = attributes.as_str();
        let self_closing = attributes.trim_end().ends_with('/');
        // Most tags are layout: only an `id` could matter on them, so their
        // attributes are not split unless one might be there.
        if Element::classify(&name) == Element::Other && !may_have_id(attributes) {
            collector.close_tag(self_closing);
            continue;
        }
        for attribute in ATTRIBUTE_RE.captures_iter(attributes) {
            let Some(attribute_name) = attribute.get(1) else {
                continue;
            };
            let value = attribute.get(2).or(attribute.get(3)).or(attribute.get(4));
            lowercase_into(&mut name, attribute_name.as_bytes());
            let value = decode_char_refs(value.map_or("", |value| value.as_str()));
            collector.attribute(&name, value.as_bytes());
        }
        collector.close_tag(self_closing);
    }
    collector.finish_with_offsets(text.as_bytes())
}

/// Whether an `id` attribute name may occur in a start tag's attribute text.
/// A false positive only costs one attribute split.
fn may_have_id(attributes: &str) -> bool {
    let bytes = attributes.as_bytes();
    memchr::memchr2_iter(b'i', b'I', bytes)
        .any(|index| matches!(bytes.get(index + 1), Some(b'd' | b'D')))
}

fn lowercase_into(buffer: &mut Vec<u8>, name: &[u8]) {
    buffer.clear();
    buffer.extend(name.iter().map(u8::to_ascii_lowercase));
}
