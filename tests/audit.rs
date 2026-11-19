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
    audit_with_robots(root, site_url, None)
}

fn audit_with_robots(root: &Path, site_url: &str, robots: Option<&Path>) -> Output {
    let mut command = audit_command(root, site_url);
    if let Some(robots) = robots {
        command.arg("--robots").arg(robots);
    }
    command.output().unwrap()
}

fn audit_command(root: &Path, site_url: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ferrolink"));
    command
        .arg("audit")
        .arg(root)
        .args(["--site-url", site_url, "--format", "json"]);
    command
}

fn report(root: &Path, site_url: &str, exit: i32) -> Value {
    parse_report(audit(root, site_url), exit)
}

fn parse_report(output: Output, exit: i32) -> Value {
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
        r#"<!doctype html><html><head><title>Ferramenta</title><link rel="canonical" href="https://ferramenta.dev/"></head><body><a href="/kit/">Kit</a></body></html>"#,
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
        "<!doctype html>\n<html><head>\n<title>Guide</title><base href=\"/ardo/guide/\"><link href=\"./\" rel=\"CANONICAL\">\n</head></html>",
    );
    write(
        root,
        "guide/setup.html",
        r#"<!doctype html><html><head><title>Setup</title><link rel="canonical" href="?a=1&amp;b=2"></head></html>"#,
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
        r#"<!doctype html><head><title>Guide</title><link rel="canonical" href="/ardo/guide/setup"></head>"#,
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
<head><title>Home</title><template><meta name="robots" content="noindex"></template></head>
<body><link rel="canonical" href="/"></body>"#,
    );
    write(
        root,
        "guide.html",
        r#"<!doctype html>
<head><title>Guide</title>
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

#[test]
fn discovers_declared_sitemaps_and_reports_the_effective_googlebot_rule() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "robots.txt",
        "User-agent: *\nDisallow: /\n\nUser-Agent: Googlebot*\nSitemap: https://example.org/search/index.xml # custom index\nUser-agent: OtherBot\nDisallow: /guide\nAllow: /guide/open$\nUser-agent: GOOGLEBOT\nAllow: /guide/tie$\nDisallow: /guide/tie$\nsitemap: https://example.org/search/guide.xml\nSitemap: https://cdn.example/search.xml\n",
    );
    write(
        root,
        "search/index.xml",
        "<sitemapindex><sitemap><loc>https://example.org/search/guide.xml</loc></sitemap></sitemapindex>",
    );
    write(
        root,
        "search/guide.xml",
        r#"<urlset>
  <url><loc>https://example.org/guide/open</loc></url>
  <url><loc>https://example.org/guide/tie</loc></url>
  <url><loc>https://example.org/guide/draft</loc></url>
  <url><loc>https://example.org/other</loc></url>
</urlset>"#,
    );
    for route in ["guide/open", "guide/tie", "guide/draft", "other"] {
        write(
            root,
            &format!("{route}.html"),
            &format!(
                r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/{route}">"#
            ),
        );
    }

    let issues = report(root, "https://example.org/", 1);
    assert_eq!(issues["summary"]["sitemaps"], 2);
    assert_eq!(
        findings(&issues),
        [("sitemap_blocked_by_robots", "robots.txt", 7)]
    );
    assert_eq!(
        issues["issues"][0]["url"],
        "https://example.org/guide/draft"
    );
    assert!(
        issues["issues"][0]["message"]
            .as_str()
            .unwrap()
            .contains("search/guide.xml:4")
    );
}

#[test]
fn reports_invalid_and_missing_local_sitemap_declarations_on_their_lines() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "robots.txt",
        "Sitemap: /custom.xml\nSitemap: https://example.org/missing.xml\n",
    );

    let issues = report(root, "https://example.org/", 1);
    assert_eq!(issues["summary"]["sitemaps"], 0);
    assert_eq!(
        findings(&issues),
        [
            ("robots_sitemap_url_invalid", "robots.txt", 1),
            ("robots_sitemap_target_missing", "robots.txt", 2),
        ]
    );
    assert_eq!(
        issues["issues"][1]["url"],
        "https://example.org/missing.xml"
    );
}

#[test]
fn subpath_audits_use_an_explicit_host_robots_file() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("build");
    let host_robots = temp.path().join("host-robots.txt");
    let site_url = "https://example.org/ardo/";
    write(&root, "robots.txt", "User-agent: *\nDisallow: /\n");
    write(
        &root,
        "sitemap.xml",
        "<urlset><url><loc>https://example.org/ardo/guide</loc></url></urlset>",
    );
    write(
        &root,
        "guide/index.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/ardo/guide">"#,
    );
    assert!(findings(&report(&root, site_url, 0)).is_empty());

    let missing = audit_with_robots(&root, site_url, Some(&host_robots));
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());

    write(
        temp.path(),
        "host-robots.txt",
        "Sitemap: https://example.org/another-site.xml\nSitemap: https://example.org/ardo/sitemap.xml\nUser-agent: *\nDisallow: /ardo/guide\nAllow: /guide\n",
    );
    let issues = parse_report(audit_with_robots(&root, site_url, Some(&host_robots)), 1);
    assert_eq!(issues["summary"]["sitemaps"], 1);
    assert_eq!(
        findings(&issues),
        [(
            "sitemap_blocked_by_robots",
            host_robots.to_str().unwrap(),
            4
        )]
    );
    assert_eq!(issues["issues"][0]["url"], "https://example.org/ardo/guide");
}

#[test]
fn reachability_follows_html_navigation_and_finds_disconnected_sitemap_pages() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let site_url = "https://example.org/docs/";
    write(
        root,
        "sitemap.xml",
        r#"<urlset>
  <url><loc>https://example.org/docs/manual/install</loc></url>
  <url><loc>https://example.org/docs/island/a</loc></url>
  <url><loc>https://example.org/docs/island/b</loc></url>
  <url><loc>https://example.org/docs/standalone</loc></url>
  <url><loc>https://example.org/docs/spec.pdf</loc></url>
</urlset>"#,
    );
    write(
        root,
        "index.html",
        r#"<!doctype html><link rel="preload" href="/docs/standalone">
<template><a href="/docs/island/a">Inert link</a></template>
<a href="http://example.org/docs/island/a">Different origin</a>
<a href="guide?entry=1#top">Guide</a>"#,
    );
    write(
        root,
        "guide/index.html",
        r#"<!doctype html><base href="/docs/manual/">
<a href="install.html#step">Installation</a>"#,
    );
    write(
        root,
        "manual/install.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/docs/manual/install">
<a href="/docs/">Back to the home page</a>"#,
    );
    write(
        root,
        "island/a.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/docs/island/a">
<a href="b">B</a>"#,
    );
    write(
        root,
        "island/b.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/docs/island/b">
<a href="a">A</a>"#,
    );
    write(
        root,
        "standalone.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/docs/standalone">"#,
    );
    write(root, "spec.pdf", "A non-HTML document.");
    assert!(findings(&report(root, site_url, 0)).is_empty());

    let output = audit_command(root, site_url)
        .arg("--check-reachability")
        .output()
        .unwrap();
    let issues = parse_report(output, 1);
    assert_eq!(
        findings(&issues),
        [
            ("sitemap_unreachable", "sitemap.xml", 3),
            ("sitemap_unreachable", "sitemap.xml", 4),
            ("sitemap_unreachable", "sitemap.xml", 5),
        ]
    );
    assert_eq!(
        issues["issues"][0]["url"],
        "https://example.org/docs/island/a"
    );

    let output = audit_command(root, site_url)
        .args([
            "--entry-point",
            "/docs/island/a",
            "--entry-point",
            "https://example.org/docs/standalone#details",
        ])
        .output()
        .unwrap();
    assert!(findings(&parse_report(output, 0)).is_empty());
}

#[test]
fn reachability_validates_starting_pages_and_supports_builds_without_a_home_page() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let site_url = "https://example.org/docs/";
    write(
        root,
        "sitemap.xml",
        "<urlset><url><loc>https://example.org/docs/guide</loc></url></urlset>",
    );
    write(
        root,
        "guide/index.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="https://example.org/docs/guide">"#,
    );

    let output = audit_command(root, site_url)
        .arg("--check-reachability")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pass --entry-point"));

    for entry in ["https://other.example/docs/guide", "/guide", "/docs/gone"] {
        let output = audit_command(root, site_url)
            .args(["--entry-point", entry])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid entry point"));
    }
    let output = audit_command(root, site_url)
        .args(["--entry-point", "/docs/guide"])
        .output()
        .unwrap();
    assert!(findings(&parse_report(output, 0)).is_empty());
}

#[test]
fn validates_lastmod_in_sitemaps_and_indexes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "sitemap-index.xml",
        r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.org/guide.xml</loc>
    <lastmod>2025-02-29</lastmod>
  </sitemap>
</sitemapindex>"#,
    );
    write(
        root,
        "guide.xml",
        r#"<urlset>
  <url><loc>https://example.org/</loc><lastmod>2024-02-29</lastmod></url>
  <url><loc>https://example.org/time.html</loc><lastmod>2024-02-29T12:00:00.123+01:00</lastmod></url>
  <url><loc>https://example.org/unzoned.html</loc>
    <lastmod>2024-02-29T12:00:00</lastmod>
  </url>
</urlset>"#,
    );
    for (file, url) in [
        ("index.html", "https://example.org/"),
        ("time.html", "https://example.org/time.html"),
        ("unzoned.html", "https://example.org/unzoned.html"),
    ] {
        write(
            root,
            file,
            &format!(r#"<!doctype html><title>Guide</title><link rel="canonical" href="{url}">"#),
        );
    }
    let issues = report(root, "https://example.org/", 1);
    assert_eq!(
        findings(&issues),
        [
            ("sitemap_lastmod_invalid", "guide.xml", 5),
            ("sitemap_lastmod_invalid", "sitemap-index.xml", 4),
        ]
    );
    assert_eq!(
        issues["issues"][0]["url"],
        "https://example.org/unzoned.html"
    );
    assert_eq!(issues["issues"][1]["url"], "https://example.org/guide.xml");
}

#[test]
fn audits_titles_and_optional_descriptions_on_sitemap_html_pages() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "sitemap.xml",
        r#"<urlset>
  <url><loc>https://example.org/</loc></url>
  <url><loc>https://example.org/missing.html</loc></url>
  <url><loc>https://example.org/no-description.html</loc></url>
</urlset>"#,
    );
    write(
        root,
        "index.html",
        r#"<!doctype html>
<head><link rel="canonical" href="/">
<title> &nbsp; </title>
<title>Guide &amp; tools</title>
<meta name="description" content=" &#10; &nbsp;">
<meta name="DESCRIPTION" content="A useful guide">
</head>"#,
    );
    write(
        root,
        "missing.html",
        r#"<!doctype html>
<head><link rel="canonical" href="/missing.html"><template><title>Inert</title></template>
<meta name="description">
</head><body><svg><title>An icon</title></svg></body>"#,
    );
    write(
        root,
        "no-description.html",
        r#"<!doctype html><title>Guide</title><link rel="canonical" href="/no-description.html">"#,
    );
    write(
        root,
        "unlisted.html",
        "<!doctype html><meta name=description>",
    );

    let issues = report(root, "https://example.org/", 1);
    assert_eq!(
        findings(&issues),
        [
            ("title_empty", "index.html", 3),
            ("title_multiple", "index.html", 4),
            ("meta_description_empty", "index.html", 5),
            ("meta_description_multiple", "index.html", 6),
            ("title_missing", "missing.html", 0),
            ("meta_description_empty", "missing.html", 3),
        ]
    );
}

#[test]
fn checks_only_embedded_json_ld_syntax_in_head_and_body() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    write(
        root,
        "sitemap.xml",
        "<urlset><url><loc>https://example.org/</loc></url></urlset>",
    );
    write(
        root,
        "index.html",
        r#"<!doctype html>
<head><title>Guide</title><link rel="canonical" href="/">
<script type="application/ld+json">[{"label":"A &amp; B","number":1e400},null]</script>
<script type=" Application/LD+JSON ; charset=utf-8 ">true</script>
</head><body>
<script
  type="application/ld+json">
  {"missing":
  }
</script>
<script type="application/ld+json">true false</script>
<script type="application/ld+json">   </script>
<script type="application/json">{ordinary data</script>
<script>window.example = "ordinary JavaScript";</script>
<template><script type="application/ld+json">not rendered</script></template>
<!-- <script type="application/ld+json">only a comment</script> -->
</body>"#,
    );
    write(
        root,
        "unlisted.html",
        r#"<script type="application/ld+json">{"bad":"\x"}</script>"#,
    );

    let issues = report(root, "https://example.org/", 1);
    assert_eq!(
        findings(&issues),
        [
            ("json_ld_syntax_invalid", "index.html", 9),
            ("json_ld_syntax_invalid", "index.html", 11),
            ("json_ld_syntax_invalid", "index.html", 12),
            ("json_ld_syntax_invalid", "unlisted.html", 1),
        ]
    );
    assert_eq!(issues["issues"][0]["url"], "https://example.org/");
    assert_eq!(
        issues["issues"][3]["url"],
        "https://example.org/unlisted.html"
    );
}

#[test]
fn audits_html_hreflang_in_a_bilingual_ardo_build() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let site_url = "https://example.org/ardo/";
    write(
        root,
        "sitemap.xml",
        r#"<urlset>
  <url><loc>https://example.org/ardo/de/guide/</loc></url>
  <url><loc>https://example.org/ardo/en/guide/</loc></url>
</urlset>"#,
    );
    write(root, "guide.pdf", "");
    let german = r#"<!doctype html>
<html lang="de"><head>
<title>Handbuch</title><link rel="canonical" href="https://example.org/ardo/de/guide/">
<link rel="alternate" hreflang="de" href="https://example.org/ardo/de/guide/">
<link rel="ALTERNATE" hreflang="EN-gb" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="x-default" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="fr" href="https://fr.example.org/guide/">
<link rel="alternate" href="/feed.xml">
<template><link rel="alternate" hreflang="bad"></template>
</head><body><link rel="alternate" hreflang="bad"></body></html>"#;
    let english = r#"<!doctype html>
<html lang="en-GB"><head>
<title>Guide</title><link rel="canonical" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="en-GB" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="de" href="https://example.org/ardo/de/guide/">
</head></html>"#;
    write(root, "de/guide/index.html", german);
    write(root, "en/guide/index.html", english);

    let clean = report(root, site_url, 0);
    assert_eq!(clean["summary"]["base_path"], "/ardo/");
    assert!(findings(&clean).is_empty());

    write(
        root,
        "de/guide/index.html",
        r#"<!doctype html>
<head><title>Handbuch</title><link rel="canonical" href="https://example.org/ardo/de/guide/">
<link rel="alternate" hreflang="de" href="https://example.org/ardo/de/guide/">
<link rel="alternate" hreflang="en-GB" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="en_UK" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="fr" href="/ardo/fr/guide/">
<link rel="alternate" hreflang="it" href="https://example.org/ardo/it/guide/">
<link rel="alternate" hreflang="es" href="https://example.org/ardo/guide.pdf">
<link rel="alternate" hreflang="ja" href="https://example.org/other/ja/guide/">
</head>"#,
    );
    write(
        root,
        "en/guide/index.html",
        r#"<!doctype html>
<head><title>Guide</title><link rel="canonical" href="https://example.org/ardo/en/guide/">
<link rel="alternate" hreflang="fr" href="https://fr.example.org/guide/">
</head><body>
<link rel="alternate" hreflang="de" href="https://example.org/ardo/de/guide/">
</body>"#,
    );

    let issues = report(root, site_url, 1);
    assert_eq!(
        findings(&issues),
        [
            ("hreflang_return_missing", "de/guide/index.html", 4),
            ("hreflang_language_invalid", "de/guide/index.html", 5),
            ("hreflang_url_invalid", "de/guide/index.html", 6),
            ("hreflang_target_missing", "de/guide/index.html", 7),
            ("hreflang_self_missing", "en/guide/index.html", 0),
        ]
    );
    assert_eq!(
        issues["issues"][0]["url"],
        "https://example.org/ardo/en/guide/"
    );
}
