//! The Ferroni backend: regular expressions over the raw text, no tokenizer.
//!
//! One pattern finds comments and start tags, so that links inside comments
//! are skipped; a second pattern splits the attributes of the tags that
//! matter. Raw-text elements (`script`, `style`, …) are recognized by their
//! start tag's name, and the scan resumes after their end tag. This is the "we
//! only need the links" approach; where it differs from a WHATWG tokenizer is
//! what the backend comparison tests measure.
//!
//! The patterns are shaped for a backtracking engine, and each choice was
//! measured on the Ardo documentation build:
//!
//! - The markup pattern has no capture groups. The tag name and the attribute
//!   text are sliced from the match, and a group-free pattern runs through
//!   Ferroni's cheaper find path instead of the capture path.
//! - The attribute part is an unrolled loop, `normal* (special normal*)*`,
//!   instead of a three-way alternation per token.
//! - End tags are not matched (nothing here needs them), and no raw-text
//!   alternative is tried at every `<`: `memmem` finds the end tag instead.
//! - Comment bodies are possessive runs of the bytes that cannot end them
//!   rather than lazy loops that try the terminator at every byte.
//! - The byte APIs skip UTF-8 validation of the document and of every match.

use std::sync::LazyLock;

use ferroni::prelude::Regex;

use super::{Collector, Element, PageScan, decode_char_refs};

/// A comment or a start tag, without capture groups.
///
/// A comment ends at `-->` or `--!>`, or at once for `<!-->` and `<!--->`,
/// and runs to the end of the input when unterminated, as in a browser. A tag
/// name ends at HTML whitespace, `/`, or `>` ([`is_name_end`]); the attribute
/// text runs to the first `>` outside quotes.
const MARKUP: &str = concat!(
    r#"<!--(?:-?>|(?:[^-]++|-(?!-!?>))*+(?:--!?>|\z))"#,
    r#"|<[a-zA-Z][^\t\n\f\r />]*+"#,
    r#"[^>"']*+(?:"[^"]*+"[^>"']*+|'[^']*+'[^>"']*+)*+>"#,
);

/// Groups: 1 name; value in 2 (double-quoted), 3 (single-quoted), or 4 (bare).
const ATTRIBUTE: &str =
    r#"([^\s"'>/=][^\s"'>/=]*+)(?:\s*+=\s*+(?:"([^"]*+)"|'([^']*+)'|([^\s>]++)))?"#;

/// Elements whose content is text up to their end tag, with scripting enabled
/// as in a browser.
const RAW_TEXT: [&[u8]; 9] = [
    b"script",
    b"style",
    b"textarea",
    b"title",
    b"xmp",
    b"iframe",
    b"noembed",
    b"noframes",
    b"noscript",
];

static MARKUP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(MARKUP).expect("the markup pattern is a valid Oniguruma regex"));
static ATTRIBUTE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(ATTRIBUTE).expect("the attribute pattern is a valid Oniguruma regex")
});

pub(super) fn scan(html: &[u8]) -> PageScan {
    let mut collector = Collector::default();
    let mut name = Vec::new();
    let mut attribute_name = Vec::new();
    // Where the current iteration started. A raw-text element ends it, and a
    // new iteration starts after the element's end tag: `find_iter` is much
    // cheaper per match than single `find_at` calls.
    let mut base = 0;
    'scan: while base < html.len() {
        for found in MARKUP_RE.find_iter_bytes(&html[base..]) {
            let tag = found.as_bytes();
            if tag.starts_with(b"<!--") {
                continue;
            }
            let start = base + found.start();
            // `<`, the name, the attribute text, `>`.
            let name_end = 1 + tag[1..]
                .iter()
                .position(|&b| is_name_end(b))
                .unwrap_or(tag.len() - 1);
            lowercase_into(&mut name, &tag[1..name_end]);
            let attributes = &tag[name_end..tag.len() - 1];
            collector.open_tag(&name, start);
            // Most tags are layout: only an `id` could matter on them, so their
            // attributes are not split unless one might be there.
            if Element::classify(&name) != Element::Other || may_have_id(attributes) {
                for attribute in ATTRIBUTE_RE.captures_iter_bytes(attributes) {
                    let Some(attribute_match) = attribute.get(1) else {
                        continue;
                    };
                    lowercase_into(&mut attribute_name, attribute_match.as_bytes());
                    let value = attribute.get(2).or(attribute.get(3)).or(attribute.get(4));
                    let value = value.map_or(&b""[..], |value| value.as_bytes());
                    if value.contains(&b'&') {
                        let text = String::from_utf8_lossy(value);
                        collector.attribute(&attribute_name, decode_char_refs(&text).as_bytes());
                    } else {
                        collector.attribute(&attribute_name, value);
                    }
                }
            }
            collector.close_tag(attributes.trim_ascii_end().ends_with(b"/"));
            if RAW_TEXT.contains(&name.as_slice()) {
                base = raw_text_end(html, base + found.end(), &name);
                continue 'scan;
            }
        }
        break;
    }
    collector.finish_with_offsets(html)
}

/// Whether `byte` ends a tag name: HTML whitespace, `/`, or `>`.
fn is_name_end(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ' | b'/' | b'>')
}

/// The position after the end tag that closes a raw-text element whose
/// content starts at `from`: `</name` (any case) followed by whitespace, `/`,
/// or `>`, up to and including the next `>`. The end of the input when there
/// is none.
fn raw_text_end(html: &[u8], from: usize, name: &[u8]) -> usize {
    let mut position = from;
    while let Some(offset) = memchr::memmem::find(&html[position..], b"</") {
        let name_start = position + offset + 2;
        let name_end = name_start + name.len();
        let closes = html
            .get(name_start..name_end)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            && html.get(name_end).is_some_and(|&next| is_name_end(next));
        if closes {
            return memchr::memchr(b'>', &html[name_end..])
                .map_or(html.len(), |gt| name_end + gt + 1);
        }
        position = name_start;
    }
    html.len()
}

/// Whether an `id` attribute may occur in a start tag's attribute text: `id`
/// after whitespace and before whitespace, `=`, `/`, or the end. A false
/// positive (inside a value) only costs one attribute split; `hidden`,
/// `width`, or `data-id` do not trigger one.
fn may_have_id(bytes: &[u8]) -> bool {
    memchr::memchr2_iter(b'i', b'I', bytes).any(|index| {
        index > 0
            && bytes[index - 1].is_ascii_whitespace()
            && matches!(bytes.get(index + 1), Some(b'd' | b'D'))
            && bytes
                .get(index + 2)
                .is_none_or(|&next| next.is_ascii_whitespace() || next == b'=' || next == b'/')
    })
}

fn lowercase_into(buffer: &mut Vec<u8>, name: &[u8]) {
    buffer.clear();
    buffer.extend(name.iter().map(u8::to_ascii_lowercase));
}
