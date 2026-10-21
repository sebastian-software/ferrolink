//! The html5gum backend: borrowed callback events, no per-token allocation.

use std::convert::Infallible;

use html5gum::emitters::callback::{CallbackEmitter, CallbackEvent};
use html5gum::{Span, Tokenizer};

use super::{Collector, PageScan};

pub(super) fn scan(html: &[u8]) -> PageScan {
    let mut collector = Collector::default();
    let mut emitter = CallbackEmitter::new(
        |event: CallbackEvent<'_>, span: Span<usize>| -> Option<Infallible> {
            match event {
                CallbackEvent::OpenStartTag { name } => collector.open_tag(name, span.start),
                CallbackEvent::AttributeName { name } => collector.attribute_name(name),
                CallbackEvent::AttributeValue { value } => collector.attribute_value(value),
                CallbackEvent::CloseStartTag { self_closing } => collector.close_tag(self_closing),
                CallbackEvent::EndTag { name } => collector.end_tag(name),
                _ => {}
            }
            None
        },
    );
    // Without tree construction, `<script>` and friends would be tokenized as
    // markup; html5gum's naive switch covers the raw-text elements.
    emitter.naively_switch_states(true);
    let Ok(()) = Tokenizer::new_with_emitter(html, emitter).finish();
    collector.finish_with_offsets(html)
}
