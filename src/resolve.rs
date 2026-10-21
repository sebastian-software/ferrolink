//! Turning an attribute value into a site-relative path, the way a browser
//! resolves it against the document URL.

use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

/// Characters escaped when a file path becomes part of a URL path.
const PATH: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

/// Where a reference points.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resolved {
    /// A path inside the site, relative to the base path and percent-decoded,
    /// with the raw fragment if there was one. `url` is the absolute URL path
    /// used in reports.
    Internal {
        path: String,
        fragment: Option<String>,
        url: String,
    },
    /// An absolute path on the same host that misses the base path, the
    /// classic mistake on project sites served below `/<repo>/`.
    OutsideBase { url: String },
    /// Another host. Not checked.
    External,
    /// A scheme without a document behind it, such as `mailto:` or `data:`.
    Skipped,
}

/// The URL every relative reference in a document resolves against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DocumentBase {
    /// An absolute URL path on the site.
    Internal(String),
    /// `<base href>` points to another host.
    External,
}

pub(crate) struct Resolver {
    base_path: String,
    site_authority: Option<String>,
}

impl Resolver {
    /// `base_path` is where the site root is served, such as `/` or `/ardo/`.
    /// `site_url` makes absolute URLs to that host count as internal.
    pub(crate) fn new(base_path: &str, site_url: Option<&str>) -> Self {
        let trimmed = base_path.trim_matches('/');
        let base_path = if trimmed.is_empty() {
            "/".to_owned()
        } else {
            format!("/{trimmed}/")
        };
        let site_authority = site_url.and_then(|url| {
            split_absolute(url).map(|(authority, _)| normalize_authority(authority))
        });
        Self {
            base_path,
            site_authority,
        }
    }

    pub(crate) fn base_path(&self) -> &str {
        &self.base_path
    }

    /// The URL path a page file is served under: `guide/index.html` becomes
    /// `/<base>/guide/`, `about.html` stays `/<base>/about.html`.
    pub(crate) fn page_url(&self, key: &str) -> String {
        let directory_index = key == "index.html"
            || key == "index.htm"
            || key.ends_with("/index.html")
            || key.ends_with("/index.htm");
        let served = if directory_index {
            &key[..key.rfind('/').map_or(0, |slash| slash + 1)]
        } else {
            key
        };
        format!("{}{}", self.base_path, utf8_percent_encode(served, PATH))
    }

    /// The base for a document's relative references, honoring `<base href>`.
    pub(crate) fn document_base(&self, page_url: &str, base_href: Option<&str>) -> DocumentBase {
        let page = DocumentBase::Internal(page_url.to_owned());
        let Some(base_href) = base_href else {
            return page;
        };
        match self.resolve(base_href, &page) {
            Resolved::Internal { url, .. } | Resolved::OutsideBase { url } => {
                DocumentBase::Internal(url)
            }
            Resolved::External => DocumentBase::External,
            Resolved::Skipped => page,
        }
    }

    pub(crate) fn resolve(&self, reference: &str, document: &DocumentBase) -> Resolved {
        let reference = clean(reference);
        if reference.is_empty() {
            return Resolved::Skipped;
        }
        if let Some(scheme_len) = scheme_len(&reference) {
            let scheme = &reference[..scheme_len];
            if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
                return self.absolute(&reference);
            }
            return Resolved::Skipped;
        }
        if reference.starts_with("//") || reference.starts_with("\\\\") {
            return self.absolute(&format!("https:{reference}"));
        }
        self.relative(&reference, document)
    }

    fn absolute(&self, url: &str) -> Resolved {
        let Some((authority, rest)) = split_absolute(url) else {
            return Resolved::External;
        };
        match &self.site_authority {
            Some(site) if *site == normalize_authority(authority) => {
                let root = DocumentBase::Internal("/".to_owned());
                if rest.is_empty() {
                    self.relative("/", &root)
                } else {
                    self.relative(rest, &root)
                }
            }
            _ => Resolved::External,
        }
    }

    fn relative(&self, reference: &str, document: &DocumentBase) -> Resolved {
        let (before_fragment, fragment) = match reference.split_once('#') {
            Some((before, fragment)) => (before, Some(fragment)),
            None => (reference, None),
        };
        let path = before_fragment
            .split_once('?')
            .map_or(before_fragment, |(path, _)| path);
        let path = if path.contains('\\') {
            Cow::Owned(path.replace('\\', "/"))
        } else {
            Cow::Borrowed(path)
        };
        let absolute = if path.starts_with('/') {
            path.into_owned()
        } else {
            let DocumentBase::Internal(base) = document else {
                return Resolved::External;
            };
            if path.is_empty() {
                base.clone()
            } else {
                let directory = &base[..base.rfind('/').map_or(0, |slash| slash + 1)];
                format!("{directory}{path}")
            }
        };
        let url = remove_dot_segments(&absolute).into_owned();
        let site_relative = if let Some(rest) = url.strip_prefix(&self.base_path) {
            rest
        } else if self.base_path.len() > 1 && url == self.base_path[..self.base_path.len() - 1] {
            // `/ardo` redirects to `/ardo/` on every static host.
            ""
        } else {
            return Resolved::OutsideBase { url };
        };
        let path = percent_decode_str(site_relative)
            .decode_utf8_lossy()
            .into_owned();
        Resolved::Internal {
            path,
            fragment: fragment.map(str::to_owned),
            url,
        }
    }
}

/// Strips what the URL parser strips: surrounding C0 controls and spaces, and
/// tabs and newlines anywhere.
fn clean(reference: &str) -> Cow<'_, str> {
    let trimmed = reference.trim_matches(|c: char| c <= ' ');
    if trimmed.contains(['\t', '\n', '\r']) {
        Cow::Owned(
            trimmed
                .chars()
                .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
                .collect(),
        )
    } else {
        Cow::Borrowed(trimmed)
    }
}

/// Length of a leading `scheme:`, without the colon.
fn scheme_len(reference: &str) -> Option<usize> {
    let mut chars = reference.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    for (index, c) in chars {
        match c {
            ':' => return Some(index),
            c if c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.') => {}
            _ => return None,
        }
    }
    None
}

/// Splits `scheme://authority/rest` into the authority and everything from the
/// first `/`, `?`, or `#` on.
fn split_absolute(url: &str) -> Option<(&str, &str)> {
    let after_scheme = &url[scheme_len(url)? + 1..];
    let after_slashes = after_scheme.trim_start_matches(['/', '\\']);
    let end = after_slashes
        .find(['/', '\\', '?', '#'])
        .unwrap_or(after_slashes.len());
    Some(after_slashes.split_at(end))
}

/// Lowercases the host and drops credentials and the default ports, so that
/// `HTTPS://Example.org:443` and `https://example.org` compare equal.
fn normalize_authority(authority: &str) -> String {
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host
        .strip_suffix(":443")
        .or_else(|| host.strip_suffix(":80"))
        .unwrap_or(host);
    host.to_ascii_lowercase()
}

/// Resolves `.` and `..` segments of an absolute path; `..` never climbs
/// above the root.
fn remove_dot_segments(path: &str) -> Cow<'_, str> {
    let has_dots = path
        .split('/')
        .any(|segment| segment == "." || segment == "..");
    if !has_dots {
        return Cow::Borrowed(path);
    }
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let last = segments.len() - 1;
    let mut output: Vec<&str> = Vec::with_capacity(segments.len());
    for (index, segment) in segments.into_iter().enumerate() {
        match segment {
            "." => {}
            ".." => {
                output.pop();
            }
            segment => {
                output.push(segment);
                continue;
            }
        }
        // A trailing dot segment leaves a directory, not a file.
        if index == last {
            output.push("");
        }
    }
    Cow::Owned(format!("/{}", output.join("/")))
}

#[cfg(test)]
mod tests {
    use super::{DocumentBase, Resolved, Resolver, remove_dot_segments};

    fn internal(path: &str, fragment: Option<&str>, url: &str) -> Resolved {
        Resolved::Internal {
            path: path.to_owned(),
            fragment: fragment.map(str::to_owned),
            url: url.to_owned(),
        }
    }

    fn page(url: &str) -> DocumentBase {
        DocumentBase::Internal(url.to_owned())
    }

    #[test]
    fn resolves_relative_references_against_the_page_directory() {
        let resolver = Resolver::new("/", None);
        let doc = page("/guide/setup/");
        assert_eq!(
            resolver.resolve("intro", &doc),
            internal("guide/setup/intro", None, "/guide/setup/intro")
        );
        assert_eq!(
            resolver.resolve("../api/#run", &doc),
            internal("guide/api/", Some("run"), "/guide/api/")
        );
        assert_eq!(
            resolver.resolve("#top", &doc),
            internal("guide/setup/", Some("top"), "/guide/setup/")
        );
        assert_eq!(
            resolver.resolve("?tab=1", &doc),
            internal("guide/setup/", None, "/guide/setup/")
        );
        assert_eq!(
            resolver.resolve("../../../../x", &doc),
            internal("x", None, "/x")
        );
    }

    #[test]
    fn decodes_paths_and_keeps_fragments_raw() {
        let resolver = Resolver::new("/", None);
        let doc = page("/");
        assert_eq!(
            resolver.resolve("/caf%C3%A9/a%20b#%C3%BC", &doc),
            internal("café/a b", Some("%C3%BC"), "/caf%C3%A9/a%20b")
        );
    }

    #[test]
    fn enforces_the_base_path() {
        let resolver = Resolver::new("ardo", None);
        assert_eq!(resolver.base_path(), "/ardo/");
        let doc = page("/ardo/guide/");
        assert_eq!(
            resolver.resolve("/ardo/api", &doc),
            internal("api", None, "/ardo/api")
        );
        assert_eq!(resolver.resolve("/ardo", &doc), internal("", None, "/ardo"));
        assert_eq!(
            resolver.resolve("/api", &doc),
            Resolved::OutsideBase {
                url: "/api".to_owned()
            }
        );
    }

    #[test]
    fn classifies_schemes_and_hosts() {
        let resolver = Resolver::new("/docs/", Some("https://Example.org/docs/"));
        let doc = page("/docs/");
        assert_eq!(
            resolver.resolve("mailto:hi@example.org", &doc),
            Resolved::Skipped
        );
        assert_eq!(
            resolver.resolve("data:image/png;base64,AA", &doc),
            Resolved::Skipped
        );
        assert_eq!(
            resolver.resolve("https://other.org/docs/", &doc),
            Resolved::External
        );
        assert_eq!(resolver.resolve("//other.org/x", &doc), Resolved::External);
        assert_eq!(
            resolver.resolve("HTTPS://example.org:443/docs/a#b", &doc),
            internal("a", Some("b"), "/docs/a")
        );
        assert_eq!(
            resolver.resolve("https://example.org", &doc),
            Resolved::OutsideBase {
                url: "/".to_owned()
            }
        );
        assert_eq!(
            resolver.resolve(" \n/docs/\ta ", &doc),
            internal("a", None, "/docs/a")
        );
    }

    #[test]
    fn honors_base_elements() {
        let resolver = Resolver::new("/", None);
        let base = resolver.document_base("/guide/", Some("/api/v2/"));
        assert_eq!(base, page("/api/v2/"));
        assert_eq!(
            resolver.resolve("types", &base),
            internal("api/v2/types", None, "/api/v2/types")
        );
        let external = resolver.document_base("/guide/", Some("https://cdn.example.org/"));
        assert_eq!(resolver.resolve("types", &external), Resolved::External);
        assert_eq!(
            resolver.resolve("/types", &external),
            internal("types", None, "/types")
        );
    }

    #[test]
    fn maps_page_files_to_urls() {
        let resolver = Resolver::new("/ardo/", None);
        assert_eq!(resolver.page_url("index.html"), "/ardo/");
        assert_eq!(resolver.page_url("guide/index.html"), "/ardo/guide/");
        assert_eq!(resolver.page_url("about.html"), "/ardo/about.html");
        assert_eq!(
            resolver.page_url("a b/100%/index.html"),
            "/ardo/a%20b/100%25/"
        );
    }

    #[test]
    fn removes_dot_segments() {
        assert_eq!(remove_dot_segments("/a/b/../c"), "/a/c");
        assert_eq!(remove_dot_segments("/a/./b/."), "/a/b/");
        assert_eq!(remove_dot_segments("/a/.."), "/");
        assert_eq!(remove_dot_segments("/../../a"), "/a");
        assert_eq!(remove_dot_segments("/a/b"), "/a/b");
    }
}
