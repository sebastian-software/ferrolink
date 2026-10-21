//! Extracting link targets and anchor names from one HTML document.
//!
//! Every parser backend drives the same collector, so the backends differ
//! only in how they tokenize. What counts as a link, an anchor, or a `<base>`
//! is decided here, once.
//!
//! The default build contains html5gum only. The `parser-comparison` feature
//! adds the backends measured against it in `docs/parsers.md`.

#[cfg(feature = "parser-comparison")]
mod ferroni;
#[cfg(feature = "parser-comparison")]
mod html5ever;
mod html5gum;
#[cfg(feature = "parser-comparison")]
mod lol_html;

#[cfg(feature = "parser-comparison")]
use std::borrow::Cow;

use rustc_hash::FxHashSet;

/// The HTML backend used to scan pages.
///
/// All backends feed the same collector and produce identical results for
/// well-formed documents; they differ in speed and in how closely they follow
/// the tree-construction rules that switch tokenizer states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Parser {
    /// [html5gum](https://crates.io/crates/html5gum): a WHATWG tokenizer with
    /// borrowed callback events and naive raw-text state switching. The
    /// fastest backend measured, and the only one in the default build.
    #[default]
    Html5gum,
    /// [html5ever](https://crates.io/crates/html5ever): Servo's tokenizer, with
    /// raw-text and CDATA states switched by ferrolink's own sink.
    #[cfg(feature = "parser-comparison")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parser-comparison")))]
    Html5ever,
    /// [lol_html](https://crates.io/crates/lol_html): Cloudflare's streaming
    /// rewriter, driven by a CSS selector for the elements that matter.
    #[cfg(feature = "parser-comparison")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parser-comparison")))]
    LolHtml,
    /// [Ferroni](https://crates.io/crates/ferroni) regular expressions over
    /// the raw text: no tokenizer, only the patterns links appear in.
    #[cfg(feature = "parser-comparison")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parser-comparison")))]
    Ferroni,
}

impl Parser {
    /// Every backend in this build, in the order they are documented.
    pub const ALL: &[Parser] = &[
        Parser::Html5gum,
        #[cfg(feature = "parser-comparison")]
        Parser::Html5ever,
        #[cfg(feature = "parser-comparison")]
        Parser::LolHtml,
        #[cfg(feature = "parser-comparison")]
        Parser::Ferroni,
    ];

    /// The name used on the command line and in reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Parser::Html5gum => "html5gum",
            #[cfg(feature = "parser-comparison")]
            Parser::Html5ever => "html5ever",
            #[cfg(feature = "parser-comparison")]
            Parser::LolHtml => "lol-html",
            #[cfg(feature = "parser-comparison")]
            Parser::Ferroni => "ferroni",
        }
    }

    /// The backend with this [`name`](Self::name), if the build contains it.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Parser> {
        Parser::ALL
            .iter()
            .copied()
            .find(|parser| parser.name() == name)
    }
}

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

/// Scans `html` with the chosen backend.
///
/// Input is treated as UTF-8; invalid sequences are replaced rather than
/// rejected, as browsers do for a document declared as UTF-8.
#[must_use]
pub fn scan(html: &[u8], parser: Parser) -> PageScan {
    match parser {
        Parser::Html5gum => html5gum::scan(html),
        #[cfg(feature = "parser-comparison")]
        Parser::Html5ever => html5ever::scan(html),
        #[cfg(feature = "parser-comparison")]
        Parser::LolHtml => lol_html::scan(html),
        #[cfg(feature = "parser-comparison")]
        Parser::Ferroni => ferroni::scan(html),
    }
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

/// Attributes a backend has to report for an element, for backends that look
/// attributes up by name instead of iterating them.
#[cfg(feature = "parser-comparison")]
fn wanted_attributes(element: Element) -> &'static [&'static str] {
    use Element as E;
    match element {
        E::A => &["id", "href", "name"],
        E::Area | E::Base => &["id", "href"],
        E::Image | E::Use => &["id", "href", "xlink:href"],
        E::Img | E::Source => &["id", "src", "srcset"],
        E::Link => &["id", "href", "rel"],
        E::Meta => &["id", "http-equiv", "name", "property", "content"],
        E::Object => &["id", "data"],
        E::Video => &["id", "src", "poster"],
        E::Audio | E::Embed | E::Iframe | E::Script | E::Track => &["id", "src"],
        E::Other => &["id"],
    }
}

/// Backend-independent state machine fed with start tags and attributes.
///
/// Backends call [`open_tag`](Self::open_tag), then [`attribute`](Self::attribute)
/// (or the split [`attribute_name`](Self::attribute_name) /
/// [`attribute_value`](Self::attribute_value) pair) for each attribute, then
/// [`close_tag`](Self::close_tag). Positions are whatever the backend tracks;
/// [`finish`](Self::finish) turns byte offsets into lines when needed.
#[derive(Default)]
struct Collector {
    scan: PageScan,
    in_start_tag: bool,
    element: Option<Element>,
    position: usize,
    slot: Option<Slot>,
    pending: Vec<(&'static str, String)>,
    rel: String,
    http_equiv: String,
    meta_name: String,
    content: Option<String>,
    foreign_depth: usize,
    opened_foreign: bool,
    positions: Vec<usize>,
}

impl Collector {
    fn open_tag(&mut self, name: &[u8], position: usize) {
        self.in_start_tag = true;
        self.element = Some(Element::classify(name));
        self.position = position;
        self.slot = None;
        self.opened_foreign = name == b"svg" || name == b"math";
        if self.opened_foreign {
            self.foreign_depth += 1;
        }
    }

    fn end_tag(&mut self, name: &[u8]) {
        self.in_start_tag = false;
        if (name == b"svg" || name == b"math") && self.foreign_depth > 0 {
            self.foreign_depth -= 1;
        }
    }

    /// Whether the tokenizer is inside `<svg>` or `<math>`, where raw-text
    /// elements do not switch states and CDATA sections are allowed.
    #[cfg(feature = "parser-comparison")]
    fn in_foreign_content(&self) -> bool {
        self.foreign_depth > 0
    }

    #[cfg(feature = "parser-comparison")]
    fn attribute(&mut self, name: &[u8], value: &[u8]) {
        self.attribute_name(name);
        self.attribute_value(value);
    }

    fn attribute_name(&mut self, name: &[u8]) {
        self.slot = match (self.in_start_tag, self.element) {
            (true, Some(element)) => slot(element, name),
            _ => None,
        };
    }

    fn attribute_value(&mut self, value: &[u8]) {
        let Some(slot) = self.slot.take() else {
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

    fn close_tag(&mut self, self_closing: bool) {
        let element = self.element.take();
        self.in_start_tag = false;
        self.slot = None;
        if self_closing && std::mem::take(&mut self.opened_foreign) {
            // `<svg/>` opened no subtree, so no end tag will leave it.
            self.foreign_depth -= 1;
        }
        self.opened_foreign = false;
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

    /// Completes a scan whose positions are already one-based lines.
    #[cfg(feature = "parser-comparison")]
    fn finish_with_lines(mut self) -> PageScan {
        for (link, line) in self.scan.links.iter_mut().zip(&self.positions) {
            link.line = u32::try_from(*line).unwrap_or(u32::MAX);
        }
        self.scan
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

/// Decodes character references in an attribute value for backends that
/// return values as written.
///
/// Numeric references and the named references that occur in URLs are
/// decoded; other named references are kept literally. The tokenizer backends
/// decode the full WHATWG table instead.
#[cfg(feature = "parser-comparison")]
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
