//! Link sources outside HTML that decide what a deployment serves: Ardo's
//! version manifest and the redirect files of Netlify, Cloudflare Pages, and
//! Vercel.
//!
//! Each file at the site root becomes one more document whose references are
//! checked like a page's. Only destinations that are plain site paths are
//! checked; placeholders, splats, and external targets are left to the host.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::scan::RawLink;
use crate::site::Site;

/// A non-HTML file at the site root that references other paths.
pub(crate) struct DeploySource {
    pub(crate) key: &'static str,
    pub(crate) links: Vec<RawLink>,
}

/// Reads the deployment files present at `root`. A file that does not parse
/// or does not have the expected shape is someone else's format and is left
/// alone.
pub(crate) fn sources(root: &Path, site: &Site) -> std::io::Result<Vec<DeploySource>> {
    let mut sources = Vec::new();
    for (key, parse) in [
        ("versions.json", ardo_versions as fn(&str) -> Vec<RawLink>),
        ("_redirects", netlify_redirects),
        ("vercel.json", vercel_config),
    ] {
        if !site.contains(key) {
            continue;
        }
        let text = fs::read_to_string(root.join(key))?;
        let links = parse(&text);
        if !links.is_empty() {
            sources.push(DeploySource { key, links });
        }
    }
    Ok(sources)
}

/// Ardo's `versions.json`: `{ "current": "v6", "versions": [{ "path": "/v6/" }] }`.
fn ardo_versions(text: &str) -> Vec<RawLink> {
    let Ok(manifest) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let Some(versions) = manifest.get("versions").and_then(Value::as_array) else {
        return Vec::new();
    };
    let paths = versions
        .iter()
        .filter_map(|version| version.get("path")?.as_str());
    json_links(text, paths, "versions.json[path]")
}

/// `vercel.json`: destinations of `redirects` and `rewrites`.
fn vercel_config(text: &str) -> Vec<RawLink> {
    let Ok(config) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let destinations = ["redirects", "rewrites"]
        .into_iter()
        .filter_map(|section| config.get(section)?.as_array())
        .flatten()
        .filter_map(|rule| rule.get("destination")?.as_str())
        .filter(|destination| is_plain_path(destination));
    json_links(text, destinations, "vercel.json[destination]")
}

/// Netlify and Cloudflare Pages `_redirects`: `from to [status]` per line.
fn netlify_redirects(text: &str) -> Vec<RawLink> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.split_once('#').map_or(line, |(rule, _)| rule);
            let mut fields = line.split_whitespace();
            let _from = fields.next()?;
            let to = fields.next()?;
            is_plain_path(to).then(|| RawLink {
                url: to.into(),
                origin: "_redirects",
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// A destination that names one path on the site: no host, no placeholder.
fn is_plain_path(destination: &str) -> bool {
    destination.starts_with('/')
        && !destination.starts_with("//")
        && !destination.contains([':', '*', '(', '$'])
}

/// Turns JSON string values into links, locating each value's line by its
/// next quoted occurrence in the source text.
fn json_links<'a>(
    text: &str,
    values: impl Iterator<Item = &'a str>,
    origin: &'static str,
) -> Vec<RawLink> {
    let mut search_from = 0;
    values
        .map(|value| {
            let quoted = format!("\"{value}\"");
            let line = match text[search_from..].find(&quoted) {
                Some(offset) => {
                    let position = search_from + offset;
                    search_from = position + quoted.len();
                    line_at(text, position)
                }
                None => 0,
            };
            RawLink {
                url: value.into(),
                origin,
                line,
            }
        })
        .collect()
}

fn line_at(text: &str, offset: usize) -> u32 {
    let newlines = memchr::memchr_iter(b'\n', &text.as_bytes()[..offset]).count();
    u32::try_from(newlines + 1).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{ardo_versions, netlify_redirects, vercel_config};

    fn urls(links: &[crate::scan::RawLink]) -> Vec<(&str, u32)> {
        links.iter().map(|link| (&*link.url, link.line)).collect()
    }

    #[test]
    fn reads_ardo_version_paths() {
        let text = "{\n  \"current\": \"v2\",\n  \"versions\": [\n    { \"id\": \"v2\", \"path\": \"/v2/\" },\n    { \"id\": \"v1\", \"path\": \"/v1/\" }\n  ]\n}\n";
        assert_eq!(urls(&ardo_versions(text)), [("/v2/", 4), ("/v1/", 5)]);
        assert!(ardo_versions("[1, 2]").is_empty());
        assert!(ardo_versions("not json").is_empty());
    }

    #[test]
    fn reads_plain_redirect_destinations() {
        let text = "# comment\n/ /v6/ 301\n/old/* /new/:splat 301\n/blog https://blog.example.org\n/* /index.html 200\n";
        assert_eq!(
            urls(&netlify_redirects(text)),
            [("/v6/", 2), ("/index.html", 5)]
        );
    }

    #[test]
    fn reads_vercel_destinations() {
        let text = r#"{"redirects":[{"source":"/","destination":"/v6/"},{"source":"/a/:x","destination":"/b/:x"}],"rewrites":[{"source":"/(.*)","destination":"/index.html"}]}"#;
        assert_eq!(
            urls(&vercel_config(text)),
            [("/v6/", 1), ("/index.html", 1)]
        );
    }
}
