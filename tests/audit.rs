//! Exercise audit as a CI consumer, using reductions of the two build layouts.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn audit(root: &Path, site_url: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ferrolink"))
        .arg("audit")
        .arg(root)
        .args(["--site-url", site_url, "--format", "json"])
        .output()
        .unwrap()
}

fn report(root: &Path, site_url: &str, exit: i32) -> Value {
    let output = audit(root, site_url);
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).expect("audit stdout is JSON")
}

fn findings(report: &Value) -> Vec<(&str, &str, u64)> {
    report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| {
            (
                issue["code"].as_str().unwrap(),
                issue["source"].as_str().unwrap(),
                issue["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn ferramenta_lists_its_homepage_and_keeps_the_kit_out_of_search() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "index.html",
        r#"<!doctype html><html><head><link rel="canonical" href="https://ferramenta.dev/"></head><body><a href="/kit/">Kit</a></body></html>"#,
    );
    write(
        root,
        "kit/index.html",
        r#"<!doctype html><html><head><meta name="robots" content="noindex"></head></html>"#,
    );
    write(root, "404.html", "<!doctype html><title>Not found</title>");
    write(
        root,
        "sitemap.xml",
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://ferramenta.dev/</loc></url>
</urlset>"#,
    );
    let clean = report(root, "https://ferramenta.dev", 0);
    assert_eq!(clean["summary"]["pages"], 3);
    assert_eq!(clean["summary"]["sitemaps"], 1);
    assert!(findings(&clean).is_empty());

    write(
        root,
        "sitemap.xml",
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://ferramenta.dev/</loc></url>
  <url><loc>https://ferramenta.dev/kit/</loc></url>
</urlset>"#,
    );
    let issues = report(root, "https://ferramenta.dev/", 1);
    assert_eq!(findings(&issues), [("sitemap_noindex", "sitemap.xml", 3)]);
    assert_eq!(issues["issues"][0]["url"], "https://ferramenta.dev/kit/");
}

#[test]
fn ardo_under_a_subpath_walks_sitemap_indexes_and_resolves_canonicals() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let site_url = "https://example.org/ardo/";
    write(
        root,
        "sitemap-index.xml",
        r#"<s:sitemapindex xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9">
  <s:sitemap><s:loc>https://example.org/ardo/sitemaps/guide.xml</s:loc></s:sitemap>
</s:sitemapindex>"#,
    );
    write(
        root,
        "sitemaps/guide.xml",
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.org/ardo/guide/</loc></url>
  <url><loc>https://example.org/ardo/guide/setup?a=1&amp;b=2</loc></url>
</urlset>"#,
    );
    write(
        root,
        "guide/index.html",
        "<!doctype html>\n<html><head>\n<base href=\"/ardo/guide/\"><link href=\"./\" rel=\"CANONICAL\">\n</head></html>",
    );
    write(
        root,
        "guide/setup.html",
        r#"<!doctype html><html><head><link rel="canonical" href="?a=1&amp;b=2"></head></html>"#,
    );

    let clean = report(root, site_url, 0);
    assert_eq!(clean["summary"]["base_path"], "/ardo/");
    assert_eq!(clean["summary"]["sitemaps"], 2);

    write(
        root,
        "sitemaps/guide.xml",
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.org/ardo/guide/</loc></url>
  <url><loc>https://example.org/guide/</loc></url>
  <url><loc>https://example.org/ardo/gone/</loc></url>
</urlset>"#,
    );
    write(
        root,
        "guide/index.html",
        r#"<!doctype html><head><link rel="canonical" href="/ardo/guide/setup"></head>"#,
    );
    let issues = report(root, site_url, 1);
    assert_eq!(
        findings(&issues),
        [
            ("sitemap_canonical_mismatch", "sitemaps/guide.xml", 2),
            ("sitemap_url_outside_site", "sitemaps/guide.xml", 3),
            ("sitemap_target_missing", "sitemaps/guide.xml", 4),
        ]
    );
}

#[test]
fn audit_requires_a_valid_public_site_url_and_returns_a_usage_error() {
    let temp = TempDir::new().unwrap();
    let result = audit(temp.path(), "https://example.org/ardo/?part=1");
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("invalid site URL"));
}

#[test]
fn reports_missing_duplicate_and_unusable_canonicals_as_distinct_findings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "sitemap.xml",
        r#"<urlset>
  <url><loc>https://example.org/</loc></url>
  <url><loc>https://example.org/guide.html</loc></url>
  <url><loc>https://example.org/private.html</loc></url>
</urlset>"#,
    );
    write(
        root,
        "index.html",
        r#"<!doctype html>
<head><template><meta name="robots" content="noindex"></template></head>
<body><link rel="canonical" href="/"></body>"#,
    );
    write(
        root,
        "guide.html",
        r#"<!doctype html>
<head>
<link rel="canonical" href="https://example.org/gone/">
<link rel="canonical" href="">
</head>"#,
    );
    write(
        root,
        "private.html",
        r#"<!doctype html>
<head>
<meta name="googlebot" content="NONE">
<link rel="canonical" href="/private.html">
</head>"#,
    );
    write(
        root,
        "alias.html",
        r#"<!doctype html>
<head>
<link rel="canonical" href="/private.html">
</head>"#,
    );
    let issues = report(root, "https://example.org/", 1);
    assert_eq!(
        findings(&issues),
        [
            ("canonical_target_noindex", "alias.html", 3),
            ("canonical_target_missing", "guide.html", 3),
            ("canonical_multiple", "guide.html", 4),
            ("canonical_invalid", "guide.html", 4),
            ("canonical_missing", "index.html", 0),
            ("sitemap_noindex", "sitemap.xml", 4),
        ]
    );
}

#[test]
fn malformed_xml_and_sitemap_index_cycles_are_ci_findings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "sitemap.xml",
        r#"<sitemapindex>
  <sitemap><loc>https://example.org/sitemap.xml</loc></sitemap>
  <sitemap><loc>https://example.org/broken.xml</loc></sitemap>
</sitemapindex>"#,
    );
    write(
        root,
        "broken.xml",
        "<urlset>\n  <url><loc></url>\n</urlset>",
    );
    let issues = report(root, "https://example.org/", 1);
    assert_eq!(issues["summary"]["sitemaps"], 2);
    assert_eq!(
        findings(&issues),
        [
            ("sitemap_invalid", "broken.xml", 2),
            ("sitemap_index_cycle", "sitemap.xml", 2),
        ]
    );
}
