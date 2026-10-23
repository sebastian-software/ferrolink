//! The Node.js binding of ferrolink, published to npm as `ferrolink`.
//!
//! `check(options)` copies the options into a [`ferrolink::Config`] and runs
//! the check as an `AsyncTask`, so it occupies a libuv worker instead of the
//! event loop; the check itself fans out over its own rayon pool. The public
//! TypeScript API in `node/src` wraps this module, which is private.

use std::num::NonZeroUsize;

use napi::bindgen_prelude::{AsyncTask, Error, Result, Status};
use napi::{Env, Task};
use napi_derive::napi;

/// What to check and how. See `CheckOptions` in `node/src/index.ts`.
#[napi(object)]
pub struct CheckOptions {
    pub root: String,
    pub base_path: Option<String>,
    pub site_url: Option<String>,
    pub exclude: Option<Vec<String>>,
    pub assume_present: Option<Vec<String>>,
    pub anchors: Option<bool>,
    pub deploy_files: Option<bool>,
    pub threads: Option<u32>,
}

#[napi(string_enum)]
pub enum IssueKind {
    #[napi(value = "broken_link")]
    BrokenLink,
    #[napi(value = "missing_anchor")]
    MissingAnchor,
    #[napi(value = "outside_base")]
    OutsideBase,
}

#[napi(object)]
pub struct Issue {
    pub source: String,
    pub line: u32,
    pub origin: String,
    pub href: String,
    pub target: String,
    pub kind: IssueKind,
}

#[napi(object)]
pub struct Summary {
    pub pages: u32,
    pub checked_pages: u32,
    pub files: u32,
    pub internal_links: u32,
    pub external_links: u32,
    pub skipped_links: u32,
    pub issues: u32,
    pub base_path: String,
}

#[napi(object)]
pub struct Timings {
    pub walk_ms: f64,
    pub scan_ms: f64,
    pub check_ms: f64,
}

#[napi(object)]
pub struct Report {
    pub summary: Summary,
    pub issues: Vec<Issue>,
    pub timings: Timings,
}

/// Runs a check on the libuv thread pool and resolves with its report.
#[napi(ts_return_type = "Promise<Report>")]
pub fn check(options: CheckOptions) -> Result<AsyncTask<CheckTask>> {
    let threads = match options.threads {
        None => None,
        Some(threads) => Some(
            usize::try_from(threads)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| {
                    Error::new(Status::InvalidArg, "threads must be a positive integer")
                })?,
        ),
    };
    let mut config = ferrolink::Config::new(options.root);
    config.base_path = options.base_path;
    config.site_url = options.site_url;
    config.exclude = options.exclude.unwrap_or_default();
    config.assume_present = options.assume_present.unwrap_or_default();
    config.anchors = options.anchors.unwrap_or(true);
    config.deploy_files = options.deploy_files.unwrap_or(true);
    config.threads = threads;
    Ok(AsyncTask::new(CheckTask { config }))
}

pub struct CheckTask {
    config: ferrolink::Config,
}

impl Task for CheckTask {
    type Output = ferrolink::Report;
    type JsValue = Report;

    fn compute(&mut self) -> Result<Self::Output> {
        ferrolink::check(&self.config)
            .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))
    }

    fn resolve(&mut self, _env: Env, report: Self::Output) -> Result<Self::JsValue> {
        Ok(Report::from(report))
    }
}

impl From<ferrolink::Report> for Report {
    fn from(report: ferrolink::Report) -> Self {
        let s = report.summary;
        let t = report.timings;
        Report {
            summary: Summary {
                pages: count(s.pages),
                checked_pages: count(s.checked_pages),
                files: count(s.files),
                internal_links: count(s.internal_links),
                external_links: count(s.external_links),
                skipped_links: count(s.skipped_links),
                issues: count(s.issues),
                base_path: s.base_path,
            },
            issues: report.issues.into_iter().map(Issue::from).collect(),
            timings: Timings {
                walk_ms: t.walk_ms,
                scan_ms: t.scan_ms,
                check_ms: t.check_ms,
            },
        }
    }
}

impl From<ferrolink::Issue> for Issue {
    fn from(issue: ferrolink::Issue) -> Self {
        Issue {
            source: issue.source,
            line: issue.line,
            origin: issue.origin.to_owned(),
            href: issue.href,
            target: issue.target,
            kind: match issue.kind {
                ferrolink::IssueKind::BrokenLink => IssueKind::BrokenLink,
                ferrolink::IssueKind::MissingAnchor => IssueKind::MissingAnchor,
                ferrolink::IssueKind::OutsideBase => IssueKind::OutsideBase,
            },
        }
    }
}

/// A count as a JavaScript number. No build has four billion pages.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
