//! End-to-end checks of the `ferrolink` binary against `tests/fixtures/site`,
//! a small build served under `/docs/` at `https://example.org/docs/`.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const SITE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/site");

fn ferrolink(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ferrolink"))
        .args(args)
        .output()
        .expect("the ferrolink binary runs")
}

fn json_issues(args: &[&str]) -> Vec<(String, u64, String, String)> {
    let mut all = vec!["check", SITE, "--format", "json"];
    all.extend_from_slice(args);
    let output = ferrolink(&all);
    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout is a JSON report");
    report["issues"]
        .as_array()
        .expect("issues is an array")
        .iter()
        .map(|issue| {
            (
                issue["source"].as_str().unwrap().to_owned(),
                issue["line"].as_u64().unwrap(),
                issue["kind"].as_str().unwrap().to_owned(),
                issue["target"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn issue(source: &str, line: u64, kind: &str, target: &str) -> (String, u64, String, String) {
    (source.to_owned(), line, kind.to_owned(), target.to_owned())
}

#[test]
fn reports_every_kind_of_issue_with_the_site_url() {
    let issues = json_issues(&["--site-url", "https://example.org/docs/"]);
    assert_eq!(
        issues,
        [
            issue("_redirects", 2, "broken_link", "/docs/removed/"),
            issue("archive/index.html", 3, "broken_link", "/docs/gone"),
            issue(
                "index.html",
                13,
                "missing_anchor",
                "/docs/guide/setup#missing"
            ),
            issue("index.html", 14, "broken_link", "/docs/guide/nope"),
            issue("index.html", 15, "outside_base", "/guide/setup"),
            issue("index.html", 16, "broken_link", "/docs/about"),
            issue("index.html", 19, "broken_link", "/docs/assets/logo@2x.svg"),
            issue("versions.json", 5, "broken_link", "/docs/v1/"),
        ]
    );
}

#[test]
fn counts_absolute_urls_as_external_without_a_site_url() {
    let issues = json_issues(&["--base", "/docs/"]);
    assert!(
        !issues
            .iter()
            .any(|(_, _, _, target)| target == "/docs/about")
    );
    assert_eq!(issues.len(), 7);
}

/// Real pages of the Ardo documentation build: every backend extracts the
/// same links, origins, and anchors as the html5ever reference.
#[cfg(feature = "parser-comparison")]
#[test]
fn every_parser_scans_real_pages_identically() {
    use ferrolink::Parser;
    use ferrolink::scan::scan;

    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ardo");
    for page in ["what-is-ardo.html", "component-playground.html"] {
        let html = std::fs::read(Path::new(dir).join(page)).unwrap();
        let reference = scan(&html, Parser::Html5ever);
        assert!(
            reference.links.len() > 50,
            "{page}: {} links",
            reference.links.len()
        );
        for &parser in Parser::ALL {
            let scanned = scan(&html, parser);
            let urls = |scan: &ferrolink::scan::PageScan| {
                scan.links
                    .iter()
                    .map(|link| (link.url.to_string(), link.origin))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                urls(&scanned),
                urls(&reference),
                "{page}: {}",
                parser.name()
            );
            assert_eq!(
                scanned.anchors,
                reference.anchors,
                "{page}: {}",
                parser.name()
            );
        }
    }
}

#[cfg(feature = "parser-comparison")]
#[test]
fn every_parser_finds_the_same_issues() {
    let reference = json_issues(&["--base", "/docs/", "--parser", "ferroni"]);
    for parser in ["html5gum", "html5ever", "lol-html"] {
        assert_eq!(
            json_issues(&["--base", "/docs/", "--parser", parser]),
            reference,
            "{parser}"
        );
    }
}

#[test]
fn excluded_pages_are_scanned_but_not_checked() {
    let issues = json_issues(&["--base", "/docs/", "--exclude", "archive/**"]);
    assert!(
        !issues
            .iter()
            .any(|(source, ..)| source.starts_with("archive/"))
    );
    assert_eq!(issues.len(), 6);
}

#[test]
fn deployment_files_can_be_skipped() {
    let issues = json_issues(&["--base", "/docs/", "--no-deploy-files"]);
    assert!(
        !issues
            .iter()
            .any(|(source, ..)| source == "_redirects" || source == "versions.json")
    );
}

#[test]
fn human_output_groups_issues_by_file_and_exits_with_one() {
    let output = ferrolink(&["check", SITE, "--base", "/docs/"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(
        "index.html\n  line 13    missing anchor  guide/setup#missing → /docs/guide/setup#missing\n"
    ));
    assert!(stdout.contains("  line 15    outside base    /guide/setup\n"));
    assert!(
        stdout.contains("7 issues in 4 files · 4 pages ·"),
        "{stdout}"
    );
}

#[test]
fn a_clean_site_exits_with_zero() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "index.html",
        r##"<a href="about#team">About</a><img src="logo.png">"##,
    );
    write(
        dir.path(),
        "about.html",
        r#"<h2 id="team">Team</h2><a href="/">Home</a>"#,
    );
    write(dir.path(), "logo.png", "");
    let output = ferrolink(&["check", dir.path().to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("No issues · 2 pages ·")
    );
}

#[test]
fn a_missing_directory_exits_with_two() {
    let output = ferrolink(&["check", "/definitely/not/here"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("error: cannot check /definitely/not/here")
    );
}

fn write(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).unwrap();
}
