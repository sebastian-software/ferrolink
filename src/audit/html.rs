//! SEO metadata from the HTML delivered by the build.

use std::convert::Infallible;

use html5gum::{DefaultEmitter, StartTag, Token, Tokenizer};

use super::{Lines, open_graph::OpenGraph};

#[derive(Default)]
pub(super) struct Metadata {
    pub base: Option<String>,
    pub canonicals: Vec<Canonical>,
    pub alternates: Vec<Alternate>,
    pub noindex: bool,
    pub links: Vec<String>,
    pub titles: Vec<TextElement>,
    pub descriptions: Vec<TextElement>,
    pub json_ld_errors: Vec<JsonLdError>,
    pub open_graph: OpenGraph,
}

pub(super) struct Canonical {
    pub href: String,
    pub line: u32,
}

pub(super) struct Alternate {
    pub language: String,
    pub href: String,
    pub line: u32,
}

pub(super) struct TextElement {
    pub line: u32,
    pub empty: bool,
}

pub(super) struct JsonLdError {
    pub line: u32,
    pub message: String,
}

struct JsonLdBlock {
    body_line: u32,
    content: Vec<u8>,
}

impl JsonLdBlock {
    fn syntax_error(self) -> Option<JsonLdError> {
        let text = String::from_utf8_lossy(&self.content);
        let error = serde_json::from_str::<serde::de::IgnoredAny>(&text).err()?;
        let offset = u32::try_from(error.line().saturating_sub(1)).unwrap_or(u32::MAX);
        Some(JsonLdError {
            line: self.body_line.saturating_add(offset),
            message: format!("Invalid JSON-LD script body: {error}"),
        })
    }
}

pub(super) fn scan(html: &[u8], collect_links: bool) -> Metadata {
    let mut emitter = DefaultEmitter::<usize>::new_with_span();
    emitter.naively_switch_states(true);
    let mut metadata = Metadata::default();
    let mut in_head = true;
    let mut in_title = false;
    let mut json_ld: Option<JsonLdBlock> = None;
    let mut templates = 0usize;
    let lines = Lines::new(html);
    for token in Tokenizer::new_with_emitter(html, emitter) {
        let token: Result<_, Infallible> = token;
        match token.unwrap() {
            Token::StartTag(tag) if tag.name.as_slice() == b"template" => templates += 1,
            Token::EndTag(tag) if tag.name.as_slice() == b"template" => {
                templates = templates.saturating_sub(1);
            }
            _ if templates != 0 => {}
            Token::EndTag(tag) if tag.name.as_slice() == b"title" => in_title = false,
            Token::EndTag(tag) if tag.name.as_slice() == b"script" => {
                metadata
                    .json_ld_errors
                    .extend(json_ld.take().and_then(JsonLdBlock::syntax_error));
            }
            Token::String(text) => {
                if let Some(block) = &mut json_ld {
                    block.content.extend_from_slice(&text.value);
                } else if in_title && let Some(title) = metadata.titles.last_mut() {
                    title.empty &= String::from_utf8_lossy(&text.value).trim().is_empty();
                }
            }
            Token::EndTag(tag) if matches!(tag.name.as_slice(), b"head" | b"body" | b"html") => {
                in_head = false;
            }
            Token::StartTag(tag) => match tag.name.as_slice() {
                b"base" if in_head && metadata.base.is_none() => {
                    metadata.base = attr(&tag, b"href").map(|value| value.into_owned());
                }
                b"link" if in_head => {
                    let rel = attr(&tag, b"rel").unwrap_or_default();
                    if has_token(&rel, "canonical") {
                        metadata.canonicals.push(Canonical {
                            href: attr(&tag, b"href").unwrap_or_default().into_owned(),
                            line: lines.at(tag.span.start),
                        });
                    }
                    if has_token(&rel, "alternate")
                        && let Some(language) = attr(&tag, b"hreflang")
                    {
                        metadata.alternates.push(Alternate {
                            language: language.into_owned(),
                            href: attr(&tag, b"href").unwrap_or_default().into_owned(),
                            line: lines.at(tag.span.start),
                        });
                    }
                }
                b"a" | b"area" if collect_links => {
                    in_head = false;
                    if let Some(href) = attr(&tag, b"href") {
                        metadata.links.push(href.into_owned());
                    }
                }
                b"title" if in_head => {
                    metadata.titles.push(TextElement {
                        line: lines.at(tag.span.start),
                        empty: true,
                    });
                    in_title = true;
                }
                b"script" if attr(&tag, b"type").is_some_and(|value| is_json_ld(&value)) => {
                    json_ld = Some(JsonLdBlock {
                        body_line: lines.at(tag.span.end),
                        content: Vec::new(),
                    });
                }
                b"meta" => {
                    if in_head && let Some(property) = attr(&tag, b"property") {
                        metadata.open_graph.record(
                            property.trim_ascii(),
                            || attr(&tag, b"content").unwrap_or_default().into_owned(),
                            lines.at(tag.span.start),
                        );
                    }
                    let name = attr(&tag, b"name").unwrap_or_default();
                    let name = name.trim_ascii();
                    if in_head && name.eq_ignore_ascii_case("description") {
                        metadata.descriptions.push(TextElement {
                            line: lines.at(tag.span.start),
                            empty: attr(&tag, b"content")
                                .is_none_or(|content| content.trim().is_empty()),
                        });
                    }
                    if ["robots", "googlebot"]
                        .iter()
                        .any(|bot| name.eq_ignore_ascii_case(bot))
                    {
                        metadata.noindex |= attr(&tag, b"content").is_some_and(|content| {
                            content
                                .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
                                .any(|token| {
                                    token.eq_ignore_ascii_case("noindex")
                                        || token.eq_ignore_ascii_case("none")
                                })
                        });
                    }
                }
                b"html" | b"head" | b"title" | b"base" | b"link" | b"script" | b"style"
                | b"noscript" | b"noframes" => {}
                // In generated HTML, a body element or its first tag closes an omitted head.
                _ => in_head = false,
            },
            _ => {}
        }
    }
    metadata
        .json_ld_errors
        .extend(json_ld.and_then(JsonLdBlock::syntax_error));
    metadata
}

fn is_json_ld(value: &str) -> bool {
    let mime_type = value
        .split_once(';')
        .map_or(value, |(mime_type, _)| mime_type);
    mime_type
        .trim_ascii()
        .eq_ignore_ascii_case("application/ld+json")
}

fn attr<'a>(tag: &'a StartTag<usize>, name: &[u8]) -> Option<std::borrow::Cow<'a, str>> {
    tag.attributes
        .get(name)
        .map(|value| String::from_utf8_lossy(&value.value))
}

fn has_token(value: &str, token: &str) -> bool {
    value
        .split_ascii_whitespace()
        .any(|value| value.eq_ignore_ascii_case(token))
}
