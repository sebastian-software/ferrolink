//! SEO metadata from the HTML delivered by the build.

use std::convert::Infallible;

use html5gum::{DefaultEmitter, StartTag, Token, Tokenizer};

use super::Lines;

#[derive(Default)]
pub(super) struct Metadata {
    pub base: Option<String>,
    pub canonicals: Vec<Canonical>,
    pub noindex: bool,
}

pub(super) struct Canonical {
    pub href: String,
    pub line: u32,
}

pub(super) fn scan(html: &[u8]) -> Metadata {
    let mut emitter = DefaultEmitter::<usize>::new_with_span();
    emitter.naively_switch_states(true);
    let mut metadata = Metadata::default();
    let mut in_head = true;
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
            Token::EndTag(tag) if matches!(tag.name.as_slice(), b"head" | b"body" | b"html") => {
                in_head = false;
            }
            Token::StartTag(tag) => match tag.name.as_slice() {
                b"base" if in_head && metadata.base.is_none() => {
                    metadata.base = attr(&tag, b"href").map(|value| value.into_owned());
                }
                b"link" if in_head => {
                    if attr(&tag, b"rel").is_some_and(|rel| has_token(&rel, "canonical")) {
                        metadata.canonicals.push(Canonical {
                            href: attr(&tag, b"href").unwrap_or_default().into_owned(),
                            line: lines.at(tag.span.start),
                        });
                    }
                }
                b"meta" => {
                    if attr(&tag, b"name").is_some_and(|name| {
                        ["robots", "googlebot"]
                            .iter()
                            .any(|bot| name.trim_ascii().eq_ignore_ascii_case(bot))
                    }) {
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
