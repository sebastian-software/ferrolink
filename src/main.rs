use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser as _, Subcommand, ValueEnum};
use ferrolink::{Config, Report};

/// Fast, parallel link checker for static HTML builds.
#[derive(clap::Parser)]
#[command(name = "ferrolink", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check links, assets, and anchors between the pages of a build.
    ///
    /// Exits with 0 when every reference resolves, 1 when issues were found,
    /// and 2 when the check could not run.
    Check(CheckArgs),
}

#[derive(clap::Args)]
struct CheckArgs {
    /// The build output directory, such as `docs/build/client`.
    dir: PathBuf,

    /// URL path the directory is served under, such as `/ardo/` on a GitHub
    /// Pages project site. Defaults to the path of `--site-url`, else `/`.
    #[arg(long, value_name = "PATH")]
    base: Option<String>,

    /// Public URL of the site. Absolute links to its host (canonical, og:url,
    /// …) are then checked like paths instead of counted as external.
    #[arg(long, value_name = "URL")]
    site_url: Option<String>,

    /// Glob (relative to DIR) of pages whose own links are not checked, such
    /// as `v3/**` for an archived version. Repeatable.
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Glob of link targets that count as present, such as `v5/**` for an
    /// archived version deployed separately: relative to DIR, or with a
    /// leading `/` the absolute URL path, which also covers paths outside the
    /// base. Matching targets are not checked for anchors either. Repeatable.
    #[arg(long, value_name = "GLOB")]
    assume_present: Vec<String>,

    /// Do not check that fragments name an element on the target page.
    #[arg(long)]
    no_anchors: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,

    /// Worker threads (default: number of CPUs).
    #[arg(long, short = 'j', value_name = "N")]
    jobs: Option<NonZeroUsize>,

    /// Do not check `versions.json`, `_redirects`, and `vercel.json`.
    #[arg(long)]
    no_deploy_files: bool,

    /// Print the duration of each phase to stderr.
    #[arg(long)]
    timings: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Command::Check(args) = cli.command;
    let mut config = Config::new(args.dir);
    config.base_path = args.base;
    config.site_url = args.site_url;
    config.exclude = args.exclude;
    config.assume_present = args.assume_present;
    config.anchors = !args.no_anchors;
    config.threads = args.jobs;
    config.deploy_files = !args.no_deploy_files;
    let report = match ferrolink::check(&config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    if args.timings {
        let t = report.timings;
        eprintln!(
            "walk {:.1} ms · scan {:.1} ms · check {:.1} ms",
            t.walk_ms, t.scan_ms, t.check_ms
        );
    }
    let written = match args.format {
        Format::Human => write_human(&mut io::stdout().lock(), &report),
        Format::Json => write_json(&mut io::stdout().lock(), &report),
    };
    if let Err(error) = written
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        eprintln!("error: cannot write the report: {error}");
        return ExitCode::from(2);
    }
    if report.issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn write_human(out: &mut impl Write, report: &Report) -> io::Result<()> {
    let mut current_source = None;
    for issue in &report.issues {
        if current_source != Some(&issue.source) {
            if current_source.is_some() {
                writeln!(out)?;
            }
            writeln!(out, "{}", issue.source)?;
            current_source = Some(&issue.source);
        }
        let location = if issue.line == 0 {
            String::new()
        } else {
            format!("line {}", issue.line)
        };
        write!(
            out,
            "  {location:<10} {:<15} {}",
            issue.kind.label(),
            issue.href
        )?;
        if issue.target != issue.href {
            write!(out, " → {}", issue.target)?;
        }
        writeln!(out)?;
    }
    let s = &report.summary;
    if !report.issues.is_empty() {
        writeln!(out)?;
    }
    let total_ms = report.timings.walk_ms + report.timings.scan_ms + report.timings.check_ms;
    let pages_with_issues = {
        let mut sources: Vec<&str> = report.issues.iter().map(|issue| &*issue.source).collect();
        sources.dedup();
        sources.len()
    };
    let verdict = match s.issues {
        0 => "No issues".to_owned(),
        1 => format!("1 issue in {pages_with_issues} file"),
        n => format!("{n} issues in {pages_with_issues} files"),
    };
    writeln!(
        out,
        "{verdict} · {} pages · {} internal links checked · {} external not checked · {:.0} ms",
        s.pages, s.internal_links, s.external_links, total_ms
    )
}

fn write_json(out: &mut impl Write, report: &Report) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, report)?;
    writeln!(out)
}
