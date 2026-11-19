//! The check pipeline: walk the build, scan every page in parallel, then
//! resolve and check every reference in parallel.

use std::fmt;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Instant;

use ferralk::ferralk_glob::{PatternOptions, PatternSet, PatternSetError};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::deploy;
use crate::report::{Issue, IssueKind, Report, Summary, Timings};
use crate::resolve::{DocumentBase, Resolved, Resolver, url_path};
use crate::scan::{self, PageScan, RawLink};
use crate::site::Site;

/// What to check and how.
///
/// New options can arrive in any release, so a `Config` starts from
/// [`Config::new`] and sets the fields it needs.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Config {
    /// The build output directory, such as `docs/build/client`.
    pub root: PathBuf,
    /// The URL path the root is served under, such as `/` or `/ardo/`. When
    /// unset, the path of `site_url`, else `/`.
    pub base_path: Option<String>,
    /// The public URL of the site. Absolute URLs to its host are checked like
    /// paths; without it, every absolute URL counts as external.
    pub site_url: Option<String>,
    /// Glob patterns, relative to the root, of pages whose references are not
    /// checked. Matching pages are still scanned, so links into them keep
    /// their anchor checks.
    pub exclude: Vec<String>,
    /// Glob patterns of link targets that count as present, such as `v5/**`
    /// for an archived version that is deployed separately. A pattern with a
    /// leading `/` matches the absolute URL path, base path included, and
    /// also covers targets outside the base path; any other pattern is
    /// relative to the root. A pattern matches the target path or the
    /// `index.html` a host serves for it. A matching target is reported
    /// neither as missing nor for a missing fragment.
    pub assume_present: Vec<String>,
    /// Whether a fragment must name an element on the target page.
    pub anchors: bool,
    /// Worker threads; the number of CPUs when unset.
    pub threads: Option<NonZeroUsize>,
    /// Whether to check `versions.json`, `_redirects`, and `vercel.json`.
    pub deploy_files: bool,
}

impl Config {
    /// Defaults for a site at `root`: served from `/` unless a site URL says
    /// otherwise, with anchors and deployment files checked.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            base_path: None,
            site_url: None,
            exclude: Vec::new(),
            assume_present: Vec::new(),
            anchors: true,
            threads: None,
            deploy_files: true,
        }
    }
}

/// Why a check could not run to completion.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The root is missing or not a directory.
    Root(PathBuf, io::Error),
    /// Walking the build failed.
    Walk(String),
    /// An `exclude` pattern does not compile.
    Exclude(PatternSetError),
    /// An `assume_present` pattern does not compile.
    AssumePresent(PatternSetError),
    /// A file could not be read.
    Read(PathBuf, io::Error),
    /// The audit site URL is not a public HTTP(S) base URL.
    InvalidSiteUrl,
    /// The supplied entry point does not identify an HTML page in the build.
    InvalidAuditEntryPoint(String),
    /// The build has no root page and reachability was given no entry points.
    MissingAuditEntryPoint(String),
    /// The worker pool could not be started.
    Threads(rayon::ThreadPoolBuildError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Root(path, error) => write!(f, "cannot check {}: {error}", path.display()),
            Error::Walk(message) => write!(f, "cannot walk the site: {message}"),
            Error::Exclude(error) => write!(f, "invalid exclude pattern: {error}"),
            Error::AssumePresent(error) => write!(f, "invalid assume-present pattern: {error}"),
            Error::Read(path, error) => write!(f, "cannot read {}: {error}", path.display()),
            Error::InvalidSiteUrl => write!(f, "invalid site URL: expected an HTTP(S) base URL"),
            Error::InvalidAuditEntryPoint(entry) => write!(
                f,
                "invalid entry point {entry:?}: expected a URL or origin-relative path serving HTML in this build",
            ),
            Error::MissingAuditEntryPoint(home) => write!(
                f,
                "no HTML page at {home}: pass --entry-point to start the reachability check",
            ),
            Error::Threads(error) => write!(f, "cannot start worker threads: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Root(_, error) | Error::Read(_, error) => Some(error),
            Error::Exclude(error) | Error::AssumePresent(error) => Some(error),
            Error::Threads(error) => Some(error),
            Error::Walk(_)
            | Error::InvalidSiteUrl
            | Error::InvalidAuditEntryPoint(_)
            | Error::MissingAuditEntryPoint(_) => None,
        }
    }
}

/// Checks every page of the build at `config.root`.
///
/// # Errors
///
/// Fails when the root cannot be walked, a page cannot be read, or an
/// exclude pattern is invalid. Broken links are not errors; they are the
/// [`Report`]'s issues.
pub fn check(config: &Config) -> Result<Report, Error> {
    Site::validate_root(&config.root)?;
    let exclude = patterns(&config.exclude).map_err(Error::Exclude)?;
    let assume_present =
        AssumedTargets::new(&config.assume_present).map_err(Error::AssumePresent)?;
    let threads = config.threads.map_or_else(
        || std::thread::available_parallelism().map_or(1, NonZeroUsize::get),
        NonZeroUsize::get,
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(Error::Threads)?;
    pool.install(|| run(config, exclude.as_ref(), &assume_present, threads))
}

fn patterns(globs: &[String]) -> Result<Option<PatternSet>, PatternSetError> {
    if globs.is_empty() {
        return Ok(None);
    }
    PatternSet::new(globs, PatternOptions::walker()).map(Some)
}

fn run(
    config: &Config,
    exclude: Option<&PatternSet>,
    assume_present: &AssumedTargets,
    threads: usize,
) -> Result<Report, Error> {
    let started = Instant::now();
    let (site, pages) = Site::walk(&config.root, threads)?;
    let checked = |key: &str| !exclude.is_some_and(|set| set.is_match_glob_path(key.as_bytes()));
    let walk_done = Instant::now();

    let scans = pages
        .par_iter()
        .map(|page| {
            let path = config.root.join(&page.path);
            let html = fs::read(&path).map_err(|error| Error::Read(path, error))?;
            Ok(scan::scan(&html))
        })
        .collect::<Result<Vec<PageScan>, Error>>()?;
    let scan_done = Instant::now();

    let base_path = config
        .base_path
        .as_deref()
        .or_else(|| config.site_url.as_deref().map(url_path))
        .unwrap_or("/");
    let resolver = Resolver::new(base_path, config.site_url.as_deref());
    let page_index: FxHashMap<&str, usize> = pages
        .iter()
        .enumerate()
        .map(|(index, page)| (&*page.key, index))
        .collect();
    let checker = Checker {
        site: &site,
        resolver: &resolver,
        scans: &scans,
        page_index: &page_index,
        assume_present,
        anchors: config.anchors,
    };
    let mut outcome = pages
        .par_iter()
        .zip(&scans)
        .filter(|(page, _)| checked(&page.key))
        .map(|(page, scan)| {
            let base =
                resolver.document_base(&resolver.page_url(&page.key), scan.base_href.as_deref());
            Outcome {
                checked_pages: 1,
                ..checker.check_links(&page.key, &scan.links, &base)
            }
        })
        .reduce(Outcome::default, Outcome::merge);
    if config.deploy_files {
        let root_base = DocumentBase::Internal(resolver.base_path().to_owned());
        let sources = deploy::sources(&config.root, &site)
            .map_err(|error| Error::Read(config.root.clone(), error))?;
        for source in sources {
            outcome = outcome.merge(checker.check_links(source.key, &source.links, &root_base));
        }
    }
    outcome.issues.sort_unstable_by(|a, b| {
        (&a.source, a.line, &a.href, a.kind).cmp(&(&b.source, b.line, &b.href, b.kind))
    });
    let done = Instant::now();

    Ok(Report {
        summary: Summary {
            pages: pages.len(),
            checked_pages: outcome.checked_pages,
            files: site.len(),
            internal_links: outcome.internal,
            external_links: outcome.external,
            skipped_links: outcome.skipped,
            issues: outcome.issues.len(),
            base_path: resolver.base_path().to_owned(),
        },
        issues: outcome.issues,
        timings: Timings {
            walk_ms: millis(walk_done - started),
            scan_ms: millis(scan_done - walk_done),
            check_ms: millis(done - scan_done),
        },
    })
}

struct Checker<'a> {
    site: &'a Site,
    resolver: &'a Resolver,
    scans: &'a [PageScan],
    page_index: &'a FxHashMap<&'a str, usize>,
    assume_present: &'a AssumedTargets,
    anchors: bool,
}

#[derive(Default)]
struct Outcome {
    issues: Vec<Issue>,
    internal: usize,
    external: usize,
    skipped: usize,
    checked_pages: usize,
}

impl Outcome {
    fn merge(mut self, other: Outcome) -> Outcome {
        self.issues.extend(other.issues);
        self.internal += other.internal;
        self.external += other.external;
        self.skipped += other.skipped;
        self.checked_pages += other.checked_pages;
        self
    }
}

impl Checker<'_> {
    fn check_links(&self, source: &str, links: &[RawLink], base: &DocumentBase) -> Outcome {
        let mut outcome = Outcome::default();
        for link in links {
            let issue = |kind, target: String| Issue {
                source: source.to_owned(),
                line: link.line,
                origin: link.origin,
                href: link.url.to_string(),
                target,
                kind,
            };
            match self.resolver.resolve(&link.url, base) {
                Resolved::Skipped => outcome.skipped += 1,
                Resolved::External => outcome.external += 1,
                Resolved::OutsideBase { url } => {
                    outcome.internal += 1;
                    if !self.assume_present.covers(None, &url) {
                        outcome.issues.push(issue(IssueKind::OutsideBase, url));
                    }
                }
                Resolved::Internal {
                    path,
                    fragment,
                    url,
                } => {
                    outcome.internal += 1;
                    let Some(file) = self.site.serve(&path) else {
                        if !self.assume_present.covers(Some(&path), &url) {
                            outcome.issues.push(issue(IssueKind::BrokenLink, url));
                        }
                        continue;
                    };
                    let Some(fragment) = fragment.filter(|_| self.anchors) else {
                        continue;
                    };
                    let Some(&target) = self.page_index.get(file) else {
                        // Fragments into images, scripts, or SVG sprites are
                        // not checked.
                        continue;
                    };
                    if !has_anchor(&self.scans[target], &fragment)
                        && !self.assume_present.covers(Some(&path), &url)
                    {
                        outcome
                            .issues
                            .push(issue(IssueKind::MissingAnchor, format!("{url}#{fragment}")));
                    }
                }
            }
        }
        outcome
    }
}

/// The `assume_present` patterns: those with a leading `/` match the absolute
/// URL path, the others the path relative to the root.
struct AssumedTargets {
    relative: Option<PatternSet>,
    absolute: Option<PatternSet>,
}

impl AssumedTargets {
    fn new(globs: &[String]) -> Result<Self, PatternSetError> {
        let (absolute, relative): (Vec<&String>, Vec<&String>) =
            globs.iter().partition(|glob| glob.starts_with('/'));
        let absolute: Vec<String> = absolute
            .into_iter()
            .map(|glob| glob.trim_start_matches('/').to_owned())
            .collect();
        let relative: Vec<String> = relative.into_iter().cloned().collect();
        Ok(Self {
            relative: patterns(&relative)?,
            absolute: patterns(&absolute)?,
        })
    }

    /// Whether a target counts as present. `path` is the decoded path
    /// relative to the root, none for a target outside the base path; `url`
    /// is the absolute URL path.
    fn covers(&self, path: Option<&str>, url: &str) -> bool {
        if let (Some(set), Some(path)) = (&self.relative, path)
            && serves_match(set, path)
        {
            return true;
        }
        let Some(set) = &self.absolute else {
            return false;
        };
        let url = percent_encoding::percent_decode_str(url).decode_utf8_lossy();
        serves_match(set, url.trim_start_matches('/'))
    }
}

/// Whether `set` matches `path` or the `index.html` a host serves for it.
fn serves_match(set: &PatternSet, path: &str) -> bool {
    let directory = path.trim_end_matches('/');
    let index = if directory.is_empty() {
        "index.html".to_owned()
    } else {
        format!("{directory}/index.html")
    };
    set.is_match_glob_path(path) || set.is_match_glob_path(index)
}

/// Whether navigating to `#fragment` lands on an element, following the
/// HTML "indicated part of the document" steps.
fn has_anchor(page: &PageScan, fragment: &str) -> bool {
    // A text directive (`#id:~:text=…`) does not name an element.
    let fragment = fragment.split_once(":~:").map_or(fragment, |(id, _)| id);
    if fragment.is_empty() || page.anchors.contains(fragment) {
        return true;
    }
    let decoded = percent_encoding::percent_decode_str(fragment).decode_utf8_lossy();
    page.anchors.contains(&*decoded) || decoded.eq_ignore_ascii_case("top")
}

fn millis(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
