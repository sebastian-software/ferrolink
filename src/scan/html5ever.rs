//! The html5ever backend: Servo's tokenizer with ferrolink's own token sink.
//!
//! The tree builder is skipped on purpose. Building a DOM costs far more than
//! tokenizing, and the only tree-construction decisions that matter for
//! extracting attributes are the tokenizer state switches, which the sink makes.

use std::cell::RefCell;

use html5ever::TokenizerResult;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};

use super::{Collector, PageScan};

struct Sink(RefCell<Collector>);

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&self, token: Token, line_number: u64) -> TokenSinkResult<()> {
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        let mut collector = self.0.borrow_mut();
        let name = tag.name.as_bytes();
        if tag.kind == TagKind::EndTag {
            collector.end_tag(name);
            return TokenSinkResult::Continue;
        }
        let in_html = !collector.in_foreign_content();
        collector.open_tag(name, usize::try_from(line_number).unwrap_or(usize::MAX));
        for attribute in &tag.attrs {
            collector.attribute(attribute.name.local.as_bytes(), attribute.value.as_bytes());
        }
        collector.close_tag(tag.self_closing);
        if !in_html {
            return TokenSinkResult::Continue;
        }
        // The state switches the tree builder makes for HTML elements, with
        // scripting enabled as in a browser.
        match name {
            b"script" => TokenSinkResult::RawData(RawKind::ScriptData),
            b"style" | b"xmp" | b"iframe" | b"noembed" | b"noframes" | b"noscript" => {
                TokenSinkResult::RawData(RawKind::Rawtext)
            }
            b"textarea" | b"title" => TokenSinkResult::RawData(RawKind::Rcdata),
            b"plaintext" => TokenSinkResult::Plaintext,
            _ => TokenSinkResult::Continue,
        }
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.0.borrow().in_foreign_content()
    }
}

pub(super) fn scan(html: &[u8]) -> PageScan {
    let text = String::from_utf8_lossy(html);
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(&text));
    let tokenizer = Tokenizer::new(Sink(RefCell::default()), TokenizerOpts::default());
    // The sink never asks the tokenizer to pause, so one feed consumes the
    // input; the loop only guards that assumption.
    while !matches!(tokenizer.feed(&input), TokenizerResult::Done) {}
    tokenizer.end();
    tokenizer.sink.0.into_inner().finish_with_lines()
}
