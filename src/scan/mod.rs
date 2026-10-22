//! Extracting link targets and anchor names from one HTML document.
//!
//! The Ferroni backend finds tags and attributes with two regular expressions
//! and feeds them to the collector, which decides, once, what counts as a
//! link, an anchor, or a `<base>`. `docs/parsers.md` records how the backend
//! was chosen over the html5gum, html5ever, and lol_html tokenizers.

mod ferroni;

use std::borrow::Cow;

use rustc_hash::FxHashSet;

/// One URL reference found in a document, before resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawLink {
    /// The attribute value with character references decoded.
    pub url: Box<str>,
    /// Where it came from, such as `a[href]` or `img[srcset]`.
    pub origin: &'static str,
    /// One-based line of the element's start tag.
    pub line: u32,
}

/// Everything ferrolink needs to know about one document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageScan {
    /// Outgoing references in document order.
    pub links: Vec<RawLink>,
    /// Fragment targets: `id` attributes on any element and `name` on `<a>`.
    pub anchors: FxHashSet<Box<str>>,
    /// The `href` of the first `<base>` element that has one.
    pub base_href: Option<Box<str>>,
}

/// Scans `html`.
///
/// Input is treated as UTF-8; invalid sequences are replaced rather than
/// rejected, as browsers do for a document declared as UTF-8.
#[must_use]
pub fn scan(html: &[u8]) -> PageScan {
    ferroni::scan(html)
}

/// Elements whose attributes ferrolink reads. Everything else only
/// contributes its `id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Element {
    A,
    Area,
    Audio,
    Base,
    Embed,
    Iframe,
    Image,
    Img,
    Link,
    Meta,
    Object,
    Script,
    Source,
    Track,
    Use,
    Video,
    Other,
}

impl Element {
    fn classify(name: &[u8]) -> Self {
        match name {
            b"a" => Self::A,
            b"area" => Self::Area,
            b"audio" => Self::Audio,
            b"base" => Self::Base,
            b"embed" => Self::Embed,
            b"iframe" => Self::Iframe,
            b"image" => Self::Image,
            b"img" => Self::Img,
            b"link" => Self::Link,
            b"meta" => Self::Meta,
            b"object" => Self::Object,
            b"script" => Self::Script,
            b"source" => Self::Source,
            b"track" => Self::Track,
            b"use" => Self::Use,
            b"video" => Self::Video,
            _ => Self::Other,
        }
    }
}

/// What an attribute means on the element it appeared on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Id,
    AnchorName,
    Url(&'static str),
    Srcset(&'static str),
    Base,
    Rel,
    HttpEquiv,
    MetaName,
    Content,
}

fn slot(element: Element, attribute: &[u8]) -> Option<Slot> {
    use Element as E;
    if attribute == b"id" {
        return Some(Slot::Id);
    }
    let slot = match (element, attribute) {
        (E::A, b"href") => Slot::Url("a[href]"),
        (E::A, b"name") => Slot::AnchorName,
        (E::Area, b"href") => Slot::Url("area[href]"),
        (E::Audio, b"src") => Slot::Url("audio[src]"),
        (E::Base, b"href") => Slot::Base,
        (E::Embed, b"src") => Slot::Url("embed[src]"),
        (E::Iframe, b"src") => Slot::Url("iframe[src]"),
        (E::Image, b"href" | b"xlink:href") => Slot::Url("image[href]"),
        (E::Img, b"src") => Slot::Url("img[src]"),
        (E::Img, b"srcset") => Slot::Srcset("img[srcset]"),
        (E::Link, b"href") => Slot::Url("link[href]"),
        (E::Link, b"rel") => Slot::Rel,
        (E::Meta, b"http-equiv") => Slot::HttpEquiv,
        (E::Meta, b"name" | b"property") => Slot::MetaName,
        (E::Meta, b"content") => Slot::Content,
        (E::Object, b"data") => Slot::Url("object[data]"),
        (E::Script, b"src") => Slot::Url("script[src]"),
        (E::Source, b"src") => Slot::Url("source[src]"),
        (E::Source, b"srcset") => Slot::Srcset("source[srcset]"),
        (E::Track, b"src") => Slot::Url("track[src]"),
        (E::Use, b"href" | b"xlink:href") => Slot::Url("use[href]"),
        (E::Video, b"src") => Slot::Url("video[src]"),
        (E::Video, b"poster") => Slot::Url("video[poster]"),
        _ => return None,
    };
    Some(slot)
}

/// State machine fed with start tags and attributes.
///
/// The backend calls [`open_tag`](Self::open_tag) with the byte offset of a
/// start tag, then [`attribute`](Self::attribute) for each attribute, then
/// [`close_tag`](Self::close_tag); [`finish_with_offsets`](Self::finish_with_offsets)
/// turns the offsets into lines.
#[derive(Default)]
struct Collector {
    scan: PageScan,
    element: Option<Element>,
    position: usize,
    pending: Vec<(&'static str, String)>,
    rel: String,
    http_equiv: String,
    meta_name: String,
    content: Option<String>,
    positions: Vec<usize>,
}

impl Collector {
    fn open_tag(&mut self, name: &[u8], position: usize) {
        self.element = Some(Element::classify(name));
        self.position = position;
    }

    fn attribute(&mut self, name: &[u8], value: &[u8]) {
        let Some(slot) = self.element.and_then(|element| slot(element, name)) else {
            return;
        };
        let value = String::from_utf8_lossy(value);
        match slot {
            Slot::Id | Slot::AnchorName => {
                if !value.is_empty() {
                    self.scan.anchors.insert(value.into());
                }
            }
            Slot::Url(origin) | Slot::Srcset(origin) => {
                // `pending` keeps the slot kind in the origin; srcset origins
                // are recognised again in `close_tag`.
                self.pending.push((origin, value.into_owned()));
            }
            Slot::Base => self.pending.push(("base[href]", value.into_owned())),
            Slot::Rel => self.rel = value.into_owned(),
            Slot::HttpEquiv => self.http_equiv = value.into_owned(),
            Slot::MetaName => self.meta_name = value.into_owned(),
            Slot::Content => self.content = Some(value.into_owned()),
        }
    }

    fn close_tag(&mut self) {
        let element = self.element.take();
        match element {
            Some(Element::Link) => {
                let hint_only = self.rel.split_ascii_whitespace().any(|token| {
                    token.eq_ignore_ascii_case("preconnect")
                        || token.eq_ignore_ascii_case("dns-prefetch")
                });
                if hint_only {
                    self.pending.clear();
                }
            }
            Some(Element::Meta) => self.close_meta(),
            _ => {}
        }
        for (origin, value) in std::mem::take(&mut self.pending) {
            match origin {
                "base[href]" => {
                    if self.scan.base_href.is_none() {
                        self.scan.base_href = Some(value.into());
                    }
                }
                "img[srcset]" | "source[srcset]" => {
                    for candidate in srcset_urls(&value) {
                        self.push_link(candidate, origin);
                    }
                }
                _ => self.push_link(&value, origin),
            }
        }
        self.rel.clear();
        self.http_equiv.clear();
        self.meta_name.clear();
        self.content = None;
    }

    fn close_meta(&mut self) {
        let Some(content) = self.content.take() else {
            return;
        };
        if self.http_equiv.eq_ignore_ascii_case("refresh") {
            if let Some(url) = refresh_url(&content) {
                self.pending.push(("meta[refresh]", url.to_owned()));
            }
            return;
        }
        let origin = match self.meta_name.to_ascii_lowercase().as_str() {
            "og:image" | "og:image:url" => "meta[og:image]",
            "og:url" => "meta[og:url]",
            "twitter:image" => "meta[twitter:image]",
            _ => return,
        };
        self.pending.push((origin, content));
    }

    fn push_link(&mut self, url: &str, origin: &'static str) {
        // An empty reference points at the document itself and cannot break.
        if url.trim_ascii().is_empty() {
            return;
        }
        self.positions.push(self.position);
        self.scan.links.push(RawLink {
            url: url.into(),
            origin,
            line: 0,
        });
    }

    /// Completes a scan whose positions are byte offsets into `html`.
    fn finish_with_offsets(mut self, html: &[u8]) -> PageScan {
        let mut newlines = memchr::memchr_iter(b'\n', html);
        let mut line = 1u32;
        let mut next_newline = newlines.next();
        // Offsets arrive in document order, so one pass over the newlines
        // assigns every line.
        for (link, &offset) in self.scan.links.iter_mut().zip(&self.positions) {
            while let Some(newline) = next_newline {
                if newline >= offset {
                    break;
                }
                line = line.saturating_add(1);
                next_newline = newlines.next();
            }
            link.line = line;
        }
        self.scan
    }
}

/// Decodes character references in an attribute value.
///
/// Numeric references and the named references that occur in URLs are
/// decoded; other named references are kept literally, where a tokenizer
/// would decode the full WHATWG table.
fn decode_char_refs(value: &str) -> Cow<'_, str> {
    if !value.contains('&') {
        return Cow::Borrowed(value);
    }
    let mut decoded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        decoded.push_str(&rest[..amp]);
        rest = &rest[amp..];
        // `rest[1..=end + 1]` is the reference body plus its `;`; the longest
        // name in the table is short, so a far-away `;` is not a reference.
        let reference = rest[1..]
            .find(';')
            .filter(|&end| end <= 32)
            .map(|end| &rest[1..=end + 1]);
        let character = reference.and_then(|reference| {
            let body = &reference[..reference.len() - 1];
            if let Some(number) = body.strip_prefix('#') {
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => number.parse().ok(),
                };
                return code.and_then(char::from_u32);
            }
            match body {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "sol" => Some('/'),
                "num" => Some('#'),
                "quest" => Some('?'),
                "equals" => Some('='),
                "percnt" => Some('%'),
                _ => None,
            }
        });
        match (reference, character) {
            (Some(reference), Some(character)) => {
                decoded.push(character);
                rest = &rest[1 + reference.len()..];
            }
            _ => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    Cow::Owned(decoded)
}

/// The URL of a `<meta http-equiv="refresh" content="5; url=/next">` value.
///
/// Follows the shared declarative refresh steps: a delay, a `;` or `,`, then
/// either `url=<target>` or the bare target.
fn refresh_url(content: &str) -> Option<&str> {
    let rest = content.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ';' || c == ',');
    let rest = match rest.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("url") => {
            match rest[3..].trim_start().strip_prefix('=') {
                Some(target) => target.trim(),
                // `urlish/path` is a bare target that happens to start with "url".
                None => rest.trim_end(),
            }
        }
        _ => rest.trim_end(),
    };
    let rest = rest
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .or_else(|| rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
        .unwrap_or(rest);
    (!rest.is_empty()).then_some(rest)
}

/// Candidate URLs of a `srcset` value, following the HTML parsing algorithm
/// closely enough to keep commas inside URLs intact.
fn srcset_urls(value: &str) -> impl Iterator<Item = &str> {
    let mut rest = value;
    std::iter::from_fn(move || {
        loop {
            rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
            if rest.is_empty() {
                return None;
            }
            let end = rest
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(rest.len());
            let (candidate, after) = rest.split_at(end);
            if let Some(url) = candidate.strip_suffix(',') {
                rest = after;
                let url = url.trim_end_matches(',');
                if url.is_empty() {
                    continue;
                }
                return Some(url);
            }
            // Skip the descriptors up to the next comma outside parentheses.
            let mut depth = 0usize;
            let mut cut = after.len();
            for (index, c) in after.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth = depth.saturating_sub(1),
                    ',' if depth == 0 => {
                        cut = index + 1;
                        break;
                    }
                    _ => {}
                }
            }
            rest = &after[cut..];
            return Some(candidate);
        }
    })
}

#[cfg(test)]
mod tests;
