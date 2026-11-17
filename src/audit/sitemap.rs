//! Walk XML sitemaps and sitemap indexes through the build's file index.

use std::fs;
use std::path::Path;

use roxmltree::{Document, Node};
use rustc_hash::FxHashSet;
use url::Url;

use super::{Audit, Code, Lines, is_web_url, issue};
use crate::Error;

const NS: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";
const ROOTS: [&str; 3] = ["sitemap.xml", "sitemap-index.xml", "sitemap_index.xml"];

enum Visit {
    Enter { file: String, from: Option<Entry> },
    Leave(String),
}

struct Entry {
    source: String,
    line: u32,
    url: Url,
}

pub(super) fn check(root: &Path, audit: &mut Audit<'_>) -> Result<usize, Error> {
    let mut todo: Vec<_> = ROOTS
        .into_iter()
        .rev()
        .filter(|file| audit.site.contains(file))
        .map(|file| Visit::Enter {
            file: file.to_owned(),
            from: None,
        })
        .collect();
    if todo.is_empty() {
        audit.issues.push(issue(
            Code::SitemapMissing,
            ROOTS[0],
            0,
            None,
            "The build has no sitemap.xml, sitemap-index.xml, or sitemap_index.xml.",
        ));
    }
    let mut visited = FxHashSet::default();
    let mut active = FxHashSet::default();
    let mut urls = FxHashSet::default();
    while let Some(visit) = todo.pop() {
        let (file, from) = match visit {
            Visit::Leave(file) => {
                active.remove(&file);
                continue;
            }
            Visit::Enter { file, from } => (file, from),
        };
        if active.contains(&file) {
            if let Some(from) = from {
                audit.issues.push(issue(
                    Code::SitemapIndexCycle,
                    &from.source,
                    from.line,
                    Some(from.url.as_str()),
                    "Sitemap index points back to a sitemap already in this chain.",
                ));
            }
            continue;
        }
        if !visited.insert(file.clone()) {
            continue;
        }
        let path = root.join(&file);
        let xml = fs::read(&path).map_err(|error| Error::Read(path, error))?;
        let xml = match std::str::from_utf8(&xml) {
            Ok(xml) => xml,
            Err(_) => {
                audit.issues.push(issue(
                    Code::SitemapInvalid,
                    &file,
                    1,
                    None,
                    "Sitemap XML must be UTF-8.",
                ));
                continue;
            }
        };
        let doc = match Document::parse(xml) {
            Ok(doc) => doc,
            Err(error) => {
                audit.issues.push(issue(
                    Code::SitemapInvalid,
                    &file,
                    error.pos().row,
                    None,
                    error.to_string(),
                ));
                continue;
            }
        };
        let node = doc.root_element();
        let lines = Lines::new(xml.as_bytes());
        let (index, tag) = match node.tag_name().name() {
            "sitemapindex" if in_namespace(node) => (true, "sitemap"),
            "urlset" if in_namespace(node) => (false, "url"),
            _ => {
                audit.issues.push(issue(
                    Code::SitemapInvalid,
                    &file,
                    lines.at(node.range().start),
                    None,
                    "Expected a urlset or sitemapindex root element.",
                ));
                continue;
            }
        };
        active.insert(file.clone());
        todo.push(Visit::Leave(file.clone()));
        let mut children = Vec::new();
        let mut entries = node
            .children()
            .filter(|child| has_tag(*child, tag))
            .peekable();
        if entries.peek().is_none() {
            audit.issues.push(issue(
                Code::SitemapInvalid,
                &file,
                lines.at(node.range().start),
                None,
                format!("The sitemap has no {tag} entries."),
            ));
        }
        for child in entries {
            let Some(entry) = entry(audit, &file, child, &lines) else {
                continue;
            };
            if index {
                if let Some(child) = index_child(audit, entry) {
                    children.push(child);
                }
            } else if urls.insert(entry.url.clone()) {
                check_page(audit, &entry);
            } else {
                audit.issues.push(issue(
                    Code::SitemapDuplicateUrl,
                    &file,
                    entry.line,
                    Some(entry.url.as_str()),
                    "The same URL appears more than once in the sitemap.",
                ));
            }
        }
        todo.extend(children.into_iter().rev());
    }
    Ok(visited.len())
}

fn in_namespace(node: Node<'_, '_>) -> bool {
    node.tag_name()
        .namespace()
        .is_none_or(|namespace| namespace == NS)
}

fn has_tag(node: Node<'_, '_>, tag: &str) -> bool {
    node.has_tag_name(tag) && in_namespace(node)
}

fn entry(audit: &mut Audit<'_>, source: &str, node: Node<'_, '_>, lines: &Lines) -> Option<Entry> {
    let mut locations = node.children().filter(|child| has_tag(*child, "loc"));
    let Some(loc) = locations.next().filter(|loc| {
        locations.next().is_none() && !loc.children().any(|child| child.is_element())
    }) else {
        audit.issues.push(issue(
            Code::SitemapInvalid,
            source,
            lines.at(node.range().start),
            None,
            "Each sitemap entry must have a single loc element containing a URL.",
        ));
        return None;
    };
    let line = lines.at(loc.range().start);
    let value: String = loc
        .children()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    let value = value.trim_ascii();
    let Some(url) = Url::parse(value).ok().filter(is_web_url) else {
        audit.issues.push(issue(
            Code::SitemapUrlInvalid,
            source,
            line,
            Some(value),
            "Use an absolute HTTP(S) URL without credentials or a fragment.",
        ));
        return None;
    };
    Some(Entry {
        source: source.to_owned(),
        line,
        url,
    })
}

fn index_child(audit: &mut Audit<'_>, entry: Entry) -> Option<Visit> {
    let path = build_path(audit, &entry)?;
    if audit.site.contains(&path) {
        return Some(Visit::Enter {
            file: path.into_owned(),
            from: Some(entry),
        });
    }
    audit.issues.push(issue(
        Code::SitemapTargetMissing,
        &entry.source,
        entry.line,
        Some(entry.url.as_str()),
        "Sitemap index URL does not point to a file in the build.",
    ));
    None
}

fn check_page(audit: &mut Audit<'_>, entry: &Entry) {
    let Some(path) = build_path(audit, entry) else {
        return;
    };
    let Some(file) = audit.site.serve(&path) else {
        audit.issues.push(issue(
            Code::SitemapTargetMissing,
            &entry.source,
            entry.line,
            Some(entry.url.as_str()),
            "Sitemap URL does not resolve to a file in the build.",
        ));
        return;
    };
    // Sitemaps can also list documents such as PDFs. Only HTML exposes metadata here.
    let Some(page) = audit.pages.get_mut(file) else {
        return;
    };
    page.sitemap_url.get_or_insert_with(|| entry.url.clone());
    if page.metadata.noindex {
        audit.issues.push(issue(
            Code::SitemapNoindex,
            &entry.source,
            entry.line,
            Some(entry.url.as_str()),
            format!("Sitemap URL serves {file}, which is marked noindex."),
        ));
        return;
    }
    match &page.metadata.canonicals[..] {
        [] => audit.issues.push(issue(
            Code::CanonicalMissing,
            file,
            0,
            Some(entry.url.as_str()),
            "A page listed in the sitemap needs a canonical link in its head.",
        )),
        [canonical] => {
            if let Some(url) = page.canonical_url(&entry.url, &canonical.href)
                && url != entry.url
            {
                audit.issues.push(issue(
                    Code::SitemapCanonicalMismatch,
                    &entry.source,
                    entry.line,
                    Some(entry.url.as_str()),
                    format!("Sitemap URL serves {file}, which declares {url} as its canonical."),
                ));
            }
        }
        _ => {}
    }
}

fn build_path<'a>(audit: &mut Audit<'_>, entry: &'a Entry) -> Option<std::borrow::Cow<'a, str>> {
    let path = audit.local_path(&entry.url);
    if path.is_none() {
        audit.issues.push(issue(
            Code::SitemapUrlOutsideSite,
            &entry.source,
            entry.line,
            Some(entry.url.as_str()),
            "Sitemap URL is outside the configured site origin or base path.",
        ));
    }
    path
}
