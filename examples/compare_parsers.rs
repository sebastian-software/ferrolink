//! Compares the parser backends on a directory of HTML files: single-threaded
//! scan throughput, and every page where a backend's links or anchors differ
//! from html5ever's.
//!
//! ```text
//! cargo run --release --example compare_parsers -- <dir> [iterations]
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, fs};

use ferrolink::Parser;
use ferrolink::scan::{PageScan, scan};

fn main() {
    let mut args = env::args().skip(1);
    let Some(dir) = args.next() else {
        eprintln!("usage: compare_parsers <dir> [iterations]");
        std::process::exit(2);
    };
    let iterations: usize = args
        .next()
        .map_or(7, |n| n.parse().expect("iterations is a number"));
    let mut files = Vec::new();
    collect_html(Path::new(&dir), &mut files);
    files.sort();
    let pages: Vec<(PathBuf, Vec<u8>)> = files
        .into_iter()
        .map(|path| {
            let html = fs::read(&path).expect("page is readable");
            (path, html)
        })
        .collect();
    let bytes: usize = pages.iter().map(|(_, html)| html.len()).sum();
    println!(
        "{} pages, {:.1} MB of HTML, {iterations} iterations, one thread\n",
        pages.len(),
        mb(bytes)
    );

    let reference: Vec<PageScan> = pages
        .iter()
        .map(|(_, html)| scan(html, Parser::Html5ever))
        .collect();
    println!("| parser | median ms | MB/s | pages differing from html5ever |");
    println!("| --- | ---: | ---: | ---: |");
    for &parser in Parser::ALL {
        let mut samples = Vec::with_capacity(iterations);
        let mut scans = Vec::new();
        for _ in 0..=iterations {
            let started = Instant::now();
            scans = pages
                .iter()
                .map(|(_, html)| scan(html, parser))
                .collect::<Vec<_>>();
            samples.push(started.elapsed().as_secs_f64());
        }
        // The first round warms caches and the lazily compiled regexes.
        samples.remove(0);
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];
        let differing: Vec<usize> = (0..pages.len())
            .filter(|&index| !same(&scans[index], &reference[index]))
            .collect();
        println!(
            "| {} | {:.1} | {:.0} | {} |",
            parser.name(),
            median * 1000.0,
            mb(bytes) / median,
            differing.len()
        );
        for &index in differing.iter().take(3) {
            explain(&pages[index].0, parser, &scans[index], &reference[index]);
        }
    }
}

/// Links (URL and origin, not line: html5ever reports where a tag ends) and
/// anchors must match.
fn same(a: &PageScan, b: &PageScan) -> bool {
    a.anchors == b.anchors
        && a.base_href == b.base_href
        && a.links.len() == b.links.len()
        && a.links
            .iter()
            .zip(&b.links)
            .all(|(x, y)| x.url == y.url && x.origin == y.origin)
}

fn explain(path: &Path, parser: Parser, got: &PageScan, reference: &PageScan) {
    eprintln!("  {}: {} differs", path.display(), parser.name());
    let urls = |scan: &PageScan| {
        scan.links
            .iter()
            .map(|l| format!("{} {}", l.origin, l.url))
            .collect::<Vec<_>>()
    };
    let (got_urls, reference_urls) = (urls(got), urls(reference));
    for url in got_urls
        .iter()
        .filter(|url| !reference_urls.contains(url))
        .take(3)
    {
        eprintln!("    extra   {url}");
    }
    for url in reference_urls
        .iter()
        .filter(|url| !got_urls.contains(url))
        .take(3)
    {
        eprintln!("    missing {url}");
    }
    for anchor in got.anchors.symmetric_difference(&reference.anchors).take(3) {
        eprintln!("    anchor  {anchor}");
    }
}

fn collect_html(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory is readable") {
        let path = entry.expect("entry is readable").path();
        if path.is_dir() {
            collect_html(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "html" || ext == "htm")
        {
            files.push(path);
        }
    }
}

fn mb(bytes: usize) -> f64 {
    bytes as f64 / 1_000_000.0
}
