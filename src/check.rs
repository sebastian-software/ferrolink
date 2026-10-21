//! The check pipeline: walk the build, scan every page in parallel, then
//! resolve and check every reference in parallel.

use std::fmt;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ferralk::ferralk_glob::{PatternOptions, PatternSet, PatternSetError};
use ferralk::{WalkOptions, Walker};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::deploy;
use crate::report::{Issue, IssueKind, Report, Summary, Timings};
use crate::resolve::{DocumentBase, Resolved, Resolver};
use crate::scan::{self, PageScan, Parser, RawLink};
use crate::site::Site;

/// What to check and how.
#[derive(Clone, Debug)]
pub struct Config {
    /// The build output directory, such as `docs/build/client`.
    pub root: PathBuf,
    /// The URL path the root is served under, such as `/` or `/ardo/`.
    pub base_path: String,
    /// The public URL of the site. Absolute URLs to its host are checked like
    /// paths; without it, every absolute URL counts as external.
    pub site_url: Option<String>,
    /// Glob patterns, relative to the root, of pages whose references are not
    /// checked. Matching pages are still scanned, so links into them keep
    /// their anchor checks.
    pub exclude: Vec<String>,
    /// The HTML backend.
    pub parser: Parser,
    /// Worker threads; the number of CPUs when unset.
    pub threads: Option<NonZeroUsize>,
    /// Whether to check `versions.json`, `_redirects`, and `vercel.json`.
    pub deploy_files: bool,
}

impl Config {
    /// Defaults for a site served from `/` at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            base_path: "/".to_owned(),
            site_url: None,
            exclude: Vec::new(),
            parser: Parser::default(),
            threads: None,
            deploy_files: true,
        }
    }
}

/// Why a check could not run to completion.
#[derive(Debug)]
pub enum Error {
    /// The root is missing or not a directory.
    Root(PathBuf, io::Error),
    /// Walking the build failed.
    Walk(String),
    /// An `--exclude` pattern does not compile.
    Exclude(PatternSetError),
    /// A file could not be read.
    Read(PathBuf, io::Error),
    /// The worker pool could not be started.
    Threads(rayon::ThreadPoolBuildError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Root(path, error) => write!(f, "cannot check {}: {error}", path.display()),
            Error::Walk(message) => write!(f, "cannot walk the site: {message}"),
            Error::Exclude(error) => write!(f, "invalid exclude pattern: {error}"),
            Error::Read(path, error) => write!(f, "cannot read {}: {error}", path.display()),
            Error::Threads(error) => write!(f, "cannot start worker threads: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Root(_, error) | Error::Read(_, error) => Some(error),
            Error::Exclude(error) => Some(error),
            Error::Threads(error) => Some(error),
            Error::Walk(_) => None,
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
    let metadata =
        fs::metadata(&config.root).map_err(|error| Error::Root(config.root.clone(), error))?;
    if !metadata.is_dir() {
        let error = io::Error::new(io::ErrorKind::NotADirectory, "not a directory");
        return Err(Error::Root(config.root.clone(), error));
    }
    let exclude = if config.exclude.is_empty() {
        None
    } else {
        Some(PatternSet::new(&config.exclude, PatternOptions::walker()).map_err(Error::Exclude)?)
    };
    let threads = config.threads.map_or_else(
        || std::thread::available_parallelism().map_or(1, NonZeroUsize::get),
        NonZeroUsize::get,
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(Error::Threads)?;
    pool.install(|| run(config, exclude.as_ref(), threads))
}

struct Page {
    key: Box<str>,
    path: PathBuf,
    checked: bool,
}

fn run(config: &Config, exclude: Option<&PatternSet>, threads: usize) -> Result<Report, Error> {
    let started = Instant::now();
    let walk = Walker::new(&config.root)
        .match_hidden(true)
        .threads(threads)
        .options(WalkOptions::default().files_only(true))
        .collect()
        .map_err(|error| Error::Walk(describe(&error)))?;
    let (entries, errors) = walk.into_parts();
    if let Some(error) = errors.first() {
        return Err(Error::Walk(describe(error)));
    }
    let mut keys = Vec::with_capacity(entries.len());
    let mut pages = Vec::new();
    for entry in &entries {
        let relative = entry.relative_path();
        let key = site_key(relative);
        if is_html(&key) {
            let checked = !exclude.is_some_and(|set| set.is_match_glob_path(key.as_bytes()));
            pages.push(Page {
                key: key.clone(),
                path: relative.to_path_buf(),
                checked,
            });
        }
        keys.push(key);
    }
    // Walk order depends on thread scheduling; sorting keeps the report and
    // the scan order reproducible.
    pages.sort_unstable_by(|a, b| a.key.cmp(&b.key));
    let site = Site::new(keys);
    let walk_done = Instant::now();

    let scans = pages
        .par_iter()
        .map(|page| {
            let path = config.root.join(&page.path);
            let html = fs::read(&path).map_err(|error| Error::Read(path, error))?;
            Ok(scan::scan(&html, config.parser))
        })
        .collect::<Result<Vec<PageScan>, Error>>()?;
    let scan_done = Instant::now();

    let resolver = Resolver::new(&config.base_path, config.site_url.as_deref());
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
    };
    let mut outcome = pages
        .par_iter()
        .zip(&scans)
        .filter(|(page, _)| page.checked)
        .map(|(page, scan)| {
            let base =
                resolver.document_base(&resolver.page_url(&page.key), scan.base_href.as_deref());
            checker.check_links(&page.key, &scan.links, &base)
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
    let checked_pages = pages.iter().filter(|page| page.checked).count();
    let done = Instant::now();

    Ok(Report {
        summary: Summary {
            pages: pages.len(),
            checked_pages,
            files: site.len(),
            internal_links: outcome.internal,
            external_links: outcome.external,
            skipped_links: outcome.skipped,
            issues: outcome.issues.len(),
            base_path: resolver.base_path().to_owned(),
            parser: config.parser.name(),
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
}

#[derive(Default)]
struct Outcome {
    issues: Vec<Issue>,
    internal: usize,
    external: usize,
    skipped: usize,
}

impl Outcome {
    fn merge(mut self, other: Outcome) -> Outcome {
        self.issues.extend(other.issues);
        self.internal += other.internal;
        self.external += other.external;
        self.skipped += other.skipped;
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
                    outcome.issues.push(issue(IssueKind::OutsideBase, url));
                }
                Resolved::Internal {
                    path,
                    fragment,
                    url,
                } => {
                    outcome.internal += 1;
                    let Some(file) = self.site.serve(&path) else {
                        outcome.issues.push(issue(IssueKind::BrokenLink, url));
                        continue;
                    };
                    let Some(fragment) = fragment else {
                        continue;
                    };
                    let Some(&target) = self.page_index.get(file) else {
                        // Fragments into images, scripts, or SVG sprites are
                        // not checked.
                        continue;
                    };
                    if !has_anchor(&self.scans[target], &fragment) {
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

fn site_key(relative: &Path) -> Box<str> {
    let mut key = String::new();
    for component in relative.components() {
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(&component.as_os_str().to_string_lossy());
    }
    key.into()
}

fn is_html(key: &str) -> bool {
    let extension = key.rsplit_once('.').map_or("", |(_, extension)| extension);
    extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
}

fn describe(error: &dyn std::error::Error) -> String {
    match error.source() {
        Some(cause) => format!("{error}: {cause}"),
        None => error.to_string(),
    }
}

fn millis(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
