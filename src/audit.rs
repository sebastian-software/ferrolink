//! Offline search and crawl checks over a static build.

mod hreflang;
mod html;
mod lastmod;
mod open_graph;
mod reachability;
mod robots;
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
    RobotsSitemapUrlInvalid,
    RobotsSitemapTargetMissing,
    SitemapBlockedByRobots,
    SitemapUnreachable,
    SitemapLastmodInvalid,
    TitleMissing,
    TitleEmpty,
    TitleMultiple,
    MetaDescriptionEmpty,
    MetaDescriptionMultiple,
    JsonLdSyntaxInvalid,
    HreflangLanguageInvalid,
    HreflangUrlInvalid,
    HreflangTargetMissing,
    HreflangSelfMissing,
    HreflangReturnMissing,
    OpenGraphPropertyMissing,
    OpenGraphPropertyEmpty,
    OpenGraphUrlInvalid,
    OpenGraphCanonicalMismatch,
    OpenGraphImageMissing,
    RobotsSyntaxInvalid,
    SitemapNoIncomingLinks,
}

/// A finding in a build file or the explicitly supplied robots.txt.
///
/// Build files use paths relative to the build root.
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

/// Optional inputs for a static build audit.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Local copy of the origin's robots.txt.
    pub robots_path: Option<&'a Path>,
    /// Enables reachability and incoming-link checks. The slice adds public URLs or origin-relative
    /// paths as entry points alongside the site root.
    pub reachability: Option<&'a [String]>,
}

/// Audits a static build published at `site_url`, including its deployment path.
///
/// # Errors
///
/// Returns an error if the site URL is invalid or the build cannot be read.
pub fn run(root: &Path, site_url: &str) -> Result<Report, Error> {
    run_with_options(root, site_url, Options::default())
}

/// Audits the build with a local copy of the host's robots.txt.
///
/// By default, a build at the origin root uses its own robots.txt when present.
/// Pass a host file to use its policy for a build served under a subpath.
///
/// # Errors
///
/// Returns an error if the site URL is invalid or an input cannot be read.
pub fn run_with_robots(
    root: &Path,
    site_url: &str,
    robots_path: Option<&Path>,
) -> Result<Report, Error> {
    run_with_options(
        root,
        site_url,
        Options {
            robots_path,
            ..Options::default()
        },
    )
}

/// Audits the build with optional robots.txt and reachability inputs.
///
/// # Errors
///
/// Returns an error if the site URL or an entry point is invalid, no starting
/// page is available for reachability, or an input cannot be read.
pub fn run_with_options(
    root: &Path,
    site_url: &str,
    options: Options<'_>,
) -> Result<Report, Error> {
    let base = Url::parse(site_url)
        .ok()
        .filter(|url| is_web_url(url) && url.query().is_none())
        .ok_or(Error::InvalidSiteUrl)?;
    let resolver = Resolver::new(base.path(), None);
    Site::validate_root(root)?;
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let (site, files) = Site::walk(root, threads)?;
    let robots = match options.robots_path {
        Some(path) => Some(robots::Robots::read(path, path.display().to_string())?),
        None if resolver.base_path() == "/" && site.contains("robots.txt") => Some(
            robots::Robots::read(&root.join("robots.txt"), "robots.txt".to_owned())?,
        ),
        None => None,
    };
    let metadata = files
        .par_iter()
        .map(|file| {
            let path = root.join(&file.path);
            let html = fs::read(&path).map_err(|error| Error::Read(path, error))?;
            Ok(html::scan(&html, options.reachability.is_some()))
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
    let navigation = options
        .reachability
        .map(|entries| reachability::walk(&audit, entries))
        .transpose()?;
    let sitemaps = sitemap::check(root, &mut audit, robots.as_ref(), navigation.as_ref())?;
    if let Some(robots) = &robots {
        robots.report_syntax(&mut audit.issues);
    }
    audit.check_canonicals();
    audit.check_page_metadata();
    hreflang::check(&mut audit);
    open_graph::check(&mut audit);
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
        self.document_base(document_url)
            .join(href)
            .ok()
            .filter(is_web_url)
    }

    fn document_base<'u>(&self, document_url: &'u Url) -> Cow<'u, Url> {
        self.metadata
            .base
            .as_deref()
            .and_then(|base| document_url.join(base.trim_ascii()).ok())
            .map_or(Cow::Borrowed(document_url), Cow::Owned)
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

    fn check_page_metadata(&mut self) {
        for (&source, page) in &self.pages {
            let document_url = page.sitemap_url.as_ref().unwrap_or(&page.page_url);
            self.issues
                .extend(page.metadata.json_ld_errors.iter().map(|error| {
                    issue(
                        Code::JsonLdSyntaxInvalid,
                        source,
                        error.line,
                        Some(document_url.as_str()),
                        error.message.as_str(),
                    )
                }));
            let Some(url) = page.sitemap_url.as_ref().filter(|_| !page.metadata.noindex) else {
                continue;
            };
            let url = Some(url.as_str());
            if page.metadata.titles.is_empty() {
                self.issues.push(issue(
                    Code::TitleMissing,
                    source,
                    0,
                    url,
                    "Add a title to the head of this sitemap page.",
                ));
            }
            if let [_, extra, ..] = &page.metadata.titles[..] {
                self.issues.push(issue(
                    Code::TitleMultiple,
                    source,
                    extra.line,
                    url,
                    "Declare a single title in the page head.",
                ));
            }
            if let [_, extra, ..] = &page.metadata.descriptions[..] {
                self.issues.push(issue(
                    Code::MetaDescriptionMultiple,
                    source,
                    extra.line,
                    url,
                    "Declare at most one meta description in the page head.",
                ));
            }
            for element in page.metadata.titles.iter().filter(|title| title.empty) {
                self.issues.push(issue(
                    Code::TitleEmpty,
                    source,
                    element.line,
                    url,
                    "Add text to this title.",
                ));
            }
            for element in page
                .metadata
                .descriptions
                .iter()
                .filter(|description| description.empty)
            {
                self.issues.push(issue(
                    Code::MetaDescriptionEmpty,
                    source,
                    element.line,
                    url,
                    "Add text to this meta description or remove the tag.",
                ));
            }
        }
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

fn absolute_web_url(href: &str) -> Option<Url> {
    let href = href.trim_ascii();
    let (scheme, authority_and_path) = href.split_once("://")?;
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || authority_and_path.starts_with(['/', '\\'])
    {
        return None;
    }
    Url::parse(href).ok().filter(is_web_url)
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
