//! Check the metadata a page declares for social previews.

use super::{Audit, Code, absolute_web_url, issue};

const REQUIRED: [&str; 4] = ["og:title", "og:type", "og:image", "og:url"];

#[derive(Default)]
pub(super) struct OpenGraph {
    declared: bool,
    tags: Vec<Tag>,
}

struct Tag {
    property: &'static str,
    content: String,
    line: u32,
}

impl OpenGraph {
    pub(super) fn record(&mut self, property: &str, content: impl FnOnce() -> String, line: u32) {
        if !property
            .get(..3)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("og:"))
        {
            return;
        }
        self.declared = true;
        if let Some(property) = REQUIRED
            .into_iter()
            .chain(["og:description"])
            .find(|name| name.eq_ignore_ascii_case(property))
        {
            self.tags.push(Tag {
                property,
                content: content(),
                line,
            });
        }
    }
}

pub(super) fn check(audit: &mut Audit<'_>) {
    for (&source, page) in &audit.pages {
        let graph = &page.metadata.open_graph;
        if !graph.declared {
            continue;
        }

        let document_url = page.sitemap_url.as_ref().unwrap_or(&page.page_url);
        let canonical = match &page.metadata.canonicals[..] {
            [canonical] => page.canonical_url(document_url, &canonical.href),
            _ => None,
        };

        for property in REQUIRED {
            if !graph.tags.iter().any(|tag| tag.property == property) {
                audit.issues.push(issue(
                    Code::OpenGraphPropertyMissing,
                    source,
                    0,
                    Some(document_url.as_str()),
                    format!("Add the required Open Graph property {property}."),
                ));
            }
        }

        for tag in &graph.tags {
            if tag.content.trim().is_empty() {
                audit.issues.push(issue(
                    Code::OpenGraphPropertyEmpty,
                    source,
                    tag.line,
                    Some(document_url.as_str()),
                    format!("Give {} a nonempty content value.", tag.property),
                ));
                continue;
            }
            if !matches!(tag.property, "og:url" | "og:image") {
                continue;
            }
            let Some(url) = absolute_web_url(&tag.content) else {
                audit.issues.push(issue(
                    Code::OpenGraphUrlInvalid,
                    source,
                    tag.line,
                    Some(document_url.as_str()),
                    format!(
                        "Use an absolute HTTP(S) URL without credentials or a fragment for {}.",
                        tag.property
                    ),
                ));
                continue;
            };
            if tag.property == "og:url"
                && let Some(canonical) = &canonical
                && &url != canonical
            {
                audit.issues.push(issue(
                    Code::OpenGraphCanonicalMismatch,
                    source,
                    tag.line,
                    Some(url.as_str()),
                    format!("Make og:url match the page's canonical URL, {canonical}."),
                ));
            }
            if tag.property == "og:image"
                && let Some(path) = audit.local_path(&url)
                && !audit.site.contains(&path)
            {
                audit.issues.push(issue(
                    Code::OpenGraphImageMissing,
                    source,
                    tag.line,
                    Some(url.as_str()),
                    "This local og:image file is absent from the build.",
                ));
            }
        }
    }
}
