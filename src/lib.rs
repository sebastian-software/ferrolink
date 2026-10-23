//! Fast, parallel link checking for static HTML builds.
//!
//! ferrolink walks a build directory, parses every page in parallel, and
//! checks that each link, asset reference, and fragment points at something
//! the build actually contains, using the URL-to-file rules static hosts
//! share. It reads Ardo's `versions.json` and the redirect files of common
//! hosts as further link sources.
//!
//! ```no_run
//! let mut config = ferrolink::Config::new("docs/build/client");
//! config.base_path = Some("/ardo/".to_owned());
//! let report = ferrolink::check(&config)?;
//! for issue in &report.issues {
//!     println!("{}:{}: {} {}", issue.source, issue.line, issue.kind.label(), issue.target);
//! }
//! # Ok::<(), ferrolink::Error>(())
//! ```
//!
//! The library exists for the CLI, the npm package's binding, and their
//! tests; its API may change in any minor release before 1.0. The npm
//! package's JavaScript API is the supported programmatic interface.

mod check;
mod deploy;
mod report;
mod resolve;
pub mod scan;
mod site;

pub use check::{Config, Error, check};
pub use report::{Issue, IssueKind, Report, Summary, Timings};
