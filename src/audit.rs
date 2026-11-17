//! Offline sitemap, canonical, and noindex checks over a static build.

mod html;
mod sitemap;

use std::borrow::Cow;
use std::fs;
use std::path::Path;

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::Serialize;
use url::Url;

use crate::check::Error;
use crate::resolve::Resolver;
use crate::site::Site;
use html::Metadata;

/// The class of a finding. Its snake-case JSON spelling is the rule ID.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    SitemapMissing,
    SitemapInvalid,
    SitemapUrlInvalid,
    SitemapUrlOutsideSite,
    SitemapTargetMissing,
    SitemapDuplicateUrl,
    SitemapIndexCycle,
    SitemapNoindex,
    SitemapCanonicalMismatch,
    CanonicalMissing,
    CanonicalMultiple,
    CanonicalInvalid,
    CanonicalTargetMissing,
    CanonicalTargetNoindex,
}

/// A finding in an HTML or sitemap file, relative to the build root.
#[derive(Debug, Serialize)]
pub struct Issue {
    pub code: Code,
    pub source: String,
    /// One-based line. Zero means the element or file is absent.
    pub line: u32,
    pub url: Option<String>,
    pub message: String,
}

/// Counts for the audited build.
#[derive(Debug, Serialize)]
pub struct Summary {
    pub pages: usize,
    pub sitemaps: usize,
    pub issues: usize,
    pub base_path: String,
}

/// The audit result. Issues are sorted by source, line, rule ID, and URL.
#[derive(Debug, Serialize)]
pub struct Report {
    pub summary: Summary,
    pub issues: Vec<Issue>,
}

/// Audits a static build published at `site_url`, including its deployment path.
///
/// # Errors
///
/// Returns an error if the site URL is invalid or the build cannot be read.
pub fn run(root: &Path, site_url: &str) -> Result<Report, Error> {
    let base = Url::parse(site_url)
        .ok()
        .filter(|url| is_web_url(url) && url.query().is_none())
        .ok_or(Error::InvalidSiteUrl)?;
    let resolver = Resolver::new(base.path(), None);
    Site::validate_root(root)?;
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let (site, files) = Site::walk(root, threads)?;
    let metadata = files
        .par_iter()
        .map(|file| {
            let path = root.join(&file.path);
            let html = fs::read(&path).map_err(|error| Error::Read(path, error))?;
            Ok(html::scan(&html))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let pages: FxHashMap<_, _> = files
        .iter()
        .zip(metadata)
        .map(|(file, metadata)| {
            let mut page_url = base.clone();
            page_url.set_path(&resolver.page_url(&file.key));
            (
                &*file.key,
                Page {
                    page_url,
                    sitemap_url: None,
                    metadata,
                },
            )
        })
        .collect();
    let mut audit = Audit {
        base,
        resolver,
        site,
        pages,
        issues: Vec::new(),
    };
    let sitemaps = sitemap::check(root, &mut audit)?;
    audit.check_canonicals();
    audit.issues.sort_unstable_by(|a, b| {
        (&a.source, a.line, a.code, &a.url, &a.message)
            .cmp(&(&b.source, b.line, b.code, &b.url, &b.message))
    });
    Ok(Report {
        summary: Summary {
            pages: files.len(),
            sitemaps,
            issues: audit.issues.len(),
            base_path: audit.resolver.base_path().to_owned(),
        },
        issues: audit.issues,
    })
}

struct Page {
    page_url: Url,
    sitemap_url: Option<Url>,
    metadata: Metadata,
}

impl Page {
    fn canonical_url(&self, document_url: &Url, href: &str) -> Option<Url> {
        let href = href.trim_ascii();
        if href.is_empty() {
            return None;
        }
        let base = self
            .metadata
            .base
            .as_deref()
            .and_then(|base| document_url.join(base.trim_ascii()).ok());
        base.as_ref()
            .unwrap_or(document_url)
            .join(href)
            .ok()
            .filter(is_web_url)
    }
}

struct Audit<'a> {
    base: Url,
    resolver: Resolver,
    site: Site,
    pages: FxHashMap<&'a str, Page>,
    issues: Vec<Issue>,
}

impl Audit<'_> {
    fn local_path<'u>(&self, url: &'u Url) -> Option<Cow<'u, str>> {
        if url.origin() != self.base.origin() {
            return None;
        }
        self.resolver.site_relative_path(url.path())
    }

    fn check_canonicals(&mut self) {
        for (&source, page) in &self.pages {
            if let [_, extra, ..] = &page.metadata.canonicals[..] {
                self.issues.push(issue(
                    Code::CanonicalMultiple,
                    source,
                    extra.line,
                    None,
                    "Declare a single canonical link.",
                ));
            }
            let document_url = page.sitemap_url.as_ref().unwrap_or(&page.page_url);
            for canonical in &page.metadata.canonicals {
                let Some(url) = page.canonical_url(document_url, &canonical.href) else {
                    self.issues.push(issue(
                        Code::CanonicalInvalid,
                        source,
                        canonical.line,
                        Some(&canonical.href),
                        "Use a nonempty HTTP(S) canonical URL without credentials or a fragment.",
                    ));
                    continue;
                };
                let Some(path) = self.local_path(&url) else {
                    // Canonical consolidation can intentionally leave this deployment.
                    continue;
                };
                let (code, message) = match self.site.serve(&path) {
                    None => (
                        Code::CanonicalTargetMissing,
                        "Canonical URL does not resolve to a file in the build.",
                    ),
                    Some(file)
                        if file != source
                            && self
                                .pages
                                .get(file)
                                .is_some_and(|target| target.metadata.noindex) =>
                    {
                        (
                            Code::CanonicalTargetNoindex,
                            "Canonical URL points to a page marked noindex.",
                        )
                    }
                    _ => continue,
                };
                self.issues.push(issue(
                    code,
                    source,
                    canonical.line,
                    Some(url.as_str()),
                    message,
                ));
            }
        }
    }
}

fn issue(
    code: Code,
    source: &str,
    line: u32,
    url: Option<&str>,
    message: impl Into<String>,
) -> Issue {
    Issue {
        code,
        source: source.to_owned(),
        line,
        url: url.map(str::to_owned),
        message: message.into(),
    }
}

fn is_web_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

struct Lines(Vec<usize>);

impl Lines {
    fn new(bytes: &[u8]) -> Self {
        Self(memchr::memchr_iter(b'\n', bytes).collect())
    }

    fn at(&self, offset: usize) -> u32 {
        let line = 1 + self.0.partition_point(|&newline| newline < offset);
        u32::try_from(line).unwrap_or(u32::MAX)
    }
}
