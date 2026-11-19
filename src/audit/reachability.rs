//! Walk internal navigation links in the delivered HTML.

use rustc_hash::FxHashSet;
use url::Url;

use super::{Audit, is_web_url};
use crate::Error;

pub(super) fn walk<'a>(
    audit: &Audit<'a>,
    entry_points: &[String],
) -> Result<FxHashSet<&'a str>, Error> {
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
    let mut seen = FxHashSet::default();
    pending.retain(|key| seen.insert(*key));
    while let Some(key) = pending.pop() {
        let page = &audit.pages[key];
        let base = page.document_base(&page.page_url);
        for href in &page.metadata.links {
            if let Ok(url) = base.join(href)
                && let Some(key) = target(audit, url)
                && seen.insert(key)
            {
                pending.push(key);
            }
        }
    }
    Ok(seen)
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
