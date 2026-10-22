//! The result of a check, as data. Rendering belongs to the CLI.

use serde::Serialize;

/// What is wrong with a reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// No file is served at the target path.
    BrokenLink,
    /// The target page exists but has no element with the fragment as its
    /// `id` (or as the `name` of an `<a>`).
    MissingAnchor,
    /// An absolute path that leaves the configured base path.
    OutsideBase,
}

impl IssueKind {
    /// A short human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            IssueKind::BrokenLink => "broken link",
            IssueKind::MissingAnchor => "missing anchor",
            IssueKind::OutsideBase => "outside base",
        }
    }
}

/// One broken reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// The page or deployment file containing the reference, relative to the
    /// site root.
    pub source: String,
    /// One-based line of the element or entry; `0` when unknown.
    pub line: u32,
    /// Where the reference came from, such as `a[href]` or `_redirects`.
    pub origin: &'static str,
    /// The reference as written, with character references decoded.
    pub href: String,
    /// The resolved absolute URL path, including the fragment for anchors.
    pub target: String,
    /// What is wrong.
    pub kind: IssueKind,
}

/// Counts for the whole run.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Summary {
    /// HTML pages scanned.
    pub pages: usize,
    /// Pages whose references were checked (pages not excluded).
    pub checked_pages: usize,
    /// Files in the build, HTML or not.
    pub files: usize,
    /// References resolved to a path on the site and checked.
    pub internal_links: usize,
    /// References to other hosts, counted but not fetched.
    pub external_links: usize,
    /// References without a document behind them (`mailto:`, `data:`, …).
    pub skipped_links: usize,
    /// Number of issues found.
    pub issues: usize,
    /// The base path the site is served under.
    pub base_path: String,
}

/// Wall-clock time per phase, in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Timings {
    /// Walking the site directory.
    pub walk_ms: f64,
    /// Reading and parsing every page.
    pub scan_ms: f64,
    /// Resolving and checking every reference.
    pub check_ms: f64,
}

/// Everything a check found.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    /// Counts for the run.
    pub summary: Summary,
    /// Issues ordered by source, line, and reference.
    pub issues: Vec<Issue>,
    /// Phase durations.
    pub timings: Timings,
}
