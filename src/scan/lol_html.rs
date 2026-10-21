//! The lol_html backend: Cloudflare's streaming rewriter with a selector that
//! reaches only the elements ferrolink reads.

use std::cell::RefCell;

use lol_html::{HtmlRewriter, Settings, element};

use super::{Collector, Element, PageScan, decode_char_refs, wanted_attributes};

const SELECTOR: &str = "[id], a, area, audio, base, embed, iframe, image, img, link, meta, \
                        object, script, source, track, use, video";

pub(super) fn scan(html: &[u8]) -> PageScan {
    let collector = RefCell::new(Collector::default());
    let settings = Settings::new()
        .with_strict(false)
        .append_element_content_handler(element!(SELECTOR, |el| {
            let mut collector = collector.borrow_mut();
            let name = el.tag_name();
            collector.open_tag(name.as_bytes(), el.source_location().bytes().start);
            for attribute in wanted_attributes(Element::classify(name.as_bytes())) {
                if let Some(value) = el.get_attribute(attribute) {
                    // lol_html hands out attribute values as written.
                    let value = decode_char_refs(&value);
                    collector.attribute(attribute.as_bytes(), value.as_bytes());
                }
            }
            collector.close_tag(el.is_self_closing());
            Ok(())
        }));
    let mut rewriter = HtmlRewriter::new(settings, |_: &[u8]| {});
    // Without strict mode and with the default unlimited memory budget, the
    // rewriter has no error left to report; a failure keeps what was found.
    let _ = rewriter.write(html).and_then(|()| rewriter.end());
    collector.into_inner().finish_with_offsets(html)
}
