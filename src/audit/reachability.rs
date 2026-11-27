//! Walk internal navigation links in the delivered HTML.

use rustc_hash::FxHashSet;
use url::Url;

use super::{Audit, is_web_url};
use crate::Error;

pub(super) struct Navigation<'a> {
    pub reachable: FxHashSet<&'a str>,
    pub linked_or_entry: FxHashSet<&'a str>,
}

pub(super) fn walk<'a>(
    audit: &Audit<'a>,
    entry_points: &[String],
) -> Result<Navigation<'a>, Error> {
    let mut home = audit.base.clone();
    home.set_path(audit.resolver.base_path());
    let mut pending: Vec<_> = target(audit, home.clone()).into_iter().collect();
    for entry in entry_points {
        let url = if entry.starts_with('/') && !entry.starts_with("//") {
            audit.base.join(entry)
        } else {
            Url::parse(entry)
        };
        let key = url
            .ok()
            .and_then(|url| target(audit, url))
            .ok_or_else(|| Error::InvalidAuditEntryPoint(entry.clone()))?;
        pending.push(key);
    }
    if pending.is_empty() {
        return Err(Error::MissingAuditEntryPoint(home.to_string()));
    }

    // Key by file so aliases such as /guide/ and /guide/index.html lead to
    // the same page, and query strings and fragments do not expand the walk.
    let mut reachable = FxHashSet::default();
    pending.retain(|key| reachable.insert(*key));
    let mut linked_or_entry = reachable.clone();
    while let Some(source) = pending.pop() {
        for target in targets(audit, source) {
            if target != source {
                linked_or_entry.insert(target);
            }
            if reachable.insert(target) {
                pending.push(target);
            }
        }
    }

    // An unreachable page can still link to another page in the build.
    for &source in audit
        .pages
        .keys()
        .filter(|source| !reachable.contains(*source))
    {
        linked_or_entry.extend(targets(audit, source).filter(|&target| target != source));
    }
    Ok(Navigation {
        reachable,
        linked_or_entry,
    })
}

fn targets<'a>(audit: &Audit<'a>, source: &str) -> impl Iterator<Item = &'a str> {
    let page = &audit.pages[source];
    let base = page.document_base(&page.page_url);
    page.metadata
        .links
        .iter()
        .filter_map(move |href| target(audit, base.join(href).ok()?))
}

fn target<'a>(audit: &Audit<'a>, mut url: Url) -> Option<&'a str> {
    url.set_fragment(None);
    if !is_web_url(&url) {
        return None;
    }
    let path = audit.local_path(&url)?;
    let file = audit.site.serve(&path)?;
    audit.pages.get_key_value(file).map(|(&key, _)| key)
}
