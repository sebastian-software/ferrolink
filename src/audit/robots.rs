//! Read the host's crawl rules and sitemap declarations from a local file.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use url::{Position, Url};

use crate::Error;

/// Google uses at most the first 500 KiB of robots.txt.
const MAX_BYTES: usize = 500 * 1024;

pub(super) struct Robots {
    pub source: String,
    pub sitemaps: Vec<Declaration>,
    rules: Vec<Rule>,
}

pub(super) struct Declaration {
    pub line: u32,
    pub value: String,
}

struct Rule {
    line: u32,
    allow: bool,
    pattern: String,
}

/// Successive user-agent lines share a group. A named group takes the place
/// of the wildcard group when it matches Googlebot.
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
enum Scope {
    #[default]
    Other,
    Wildcard,
    Googlebot,
}

impl Robots {
    pub(super) fn read(path: &Path, source: String) -> Result<Self, Error> {
        let read_error = |error| Error::Read(path.to_owned(), error);
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(read_error)?
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(read_error)?;
        if bytes.len() > MAX_BYTES {
            // Leave out a partial last line at the byte limit.
            bytes.truncate(MAX_BYTES);
            let end = bytes
                .iter()
                .rposition(|byte| matches!(byte, b'\r' | b'\n'))
                .map_or(0, |position| position + 1);
            bytes.truncate(end);
        }
        Ok(Self::parse(&String::from_utf8_lossy(&bytes), source))
    }

    fn parse(text: &str, source: String) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let text = if text.contains('\r') {
            Cow::Owned(text.replace("\r\n", "\n"))
        } else {
            Cow::Borrowed(text)
        };
        let mut sitemaps = Vec::new();
        let mut wildcard = Vec::new();
        let mut specific = Vec::new();
        let mut scope = Scope::Other;
        let mut found_googlebot = false;
        let mut had_rules = false;
        for (index, line) in text.split(['\r', '\n']).enumerate() {
            let line_number = (index + 1) as u32;
            let line = line.split_once('#').map_or(line, |(line, _)| line);
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim_ascii();
            match field.trim_ascii().to_ascii_lowercase().as_str() {
                "sitemap" => sitemaps.push(Declaration {
                    line: line_number,
                    value: value.to_owned(),
                }),
                "user-agent" => {
                    if had_rules {
                        scope = Scope::Other;
                        had_rules = false;
                    }
                    scope = scope.max(user_agent_scope(value));
                    found_googlebot |= scope == Scope::Googlebot;
                }
                "allow" | "disallow" => {
                    had_rules = true;
                    // An empty directive places no restriction on a URL.
                    if scope == Scope::Other || value.is_empty() {
                        continue;
                    }
                    let rules = if scope == Scope::Googlebot {
                        &mut specific
                    } else {
                        &mut wildcard
                    };
                    rules.push(Rule {
                        line: line_number,
                        allow: field.trim_ascii().eq_ignore_ascii_case("allow"),
                        pattern: normalize(value).into_owned(),
                    });
                }
                _ => {}
            }
        }
        let mut rules = if found_googlebot { specific } else { wildcard };
        rules.sort_unstable_by_key(|rule| (Reverse(rule.pattern.len()), !rule.allow, rule.line));
        Self {
            source,
            sitemaps,
            rules,
        }
    }

    pub(super) fn blocking_line(&self, url: &Url) -> Option<u32> {
        // Googlebot applies the rules to the public path, including the query.
        let target = normalize(&url[Position::BeforePath..Position::AfterQuery]);
        self.rules
            .iter()
            .find(|rule| matches_pattern(&rule.pattern, &target))
            .filter(|rule| !rule.allow)
            .map(|rule| rule.line)
    }
}

fn user_agent_scope(value: &str) -> Scope {
    if value.split_ascii_whitespace().next() == Some("*") {
        return Scope::Wildcard;
    }
    let name = value
        .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '-' | '_')))
        .next()
        .unwrap_or_default();
    if name.eq_ignore_ascii_case("googlebot") {
        Scope::Googlebot
    } else {
        Scope::Other
    }
}

/// Match a prefix with optional wildcards. A dollar at the very end anchors
/// the final segment. Taking the earliest match for each middle segment leaves
/// the largest possible remainder for the others.
fn matches_pattern(pattern: &str, target: &str) -> bool {
    let (pattern, anchored) = pattern
        .strip_suffix('$')
        .map_or((pattern, false), |pattern| (pattern, true));
    let mut segments = pattern.split('*');
    let Some(mut target) = target.strip_prefix(segments.next().unwrap_or_default()) else {
        return false;
    };
    let Some(last) = segments.next_back() else {
        return !anchored || target.is_empty();
    };
    if anchored {
        let Some(before_end) = target.strip_suffix(last) else {
            return false;
        };
        target = before_end;
    }
    for segment in segments {
        let Some((_, remainder)) = target.split_once(segment) else {
            return false;
        };
        target = remainder;
    }
    anchored || target.contains(last)
}

/// Compare percent escapes by octet. Decode unreserved ASCII and preserve
/// escaped delimiters such as %2F so they stay distinct from a literal slash.
fn normalize(value: &str) -> Cow<'_, str> {
    if value.is_ascii() && !value.contains('%') {
        return Cow::Borrowed(value);
    }
    let mut normalized = String::with_capacity(value.len());
    let mut bytes = value.as_bytes();
    while let Some((&byte, rest)) = bytes.split_first() {
        bytes = rest;
        if byte == b'%'
            && let [high, low, rest @ ..] = rest
            && let (Some(high), Some(low)) = (
                char::from(*high).to_digit(16),
                char::from(*low).to_digit(16),
            )
        {
            let decoded = ((high << 4) | low) as u8;
            if decoded.is_ascii_alphanumeric() || matches!(decoded, b'-' | b'.' | b'_' | b'~') {
                normalized.push(char::from(decoded));
            } else {
                write!(normalized, "%{decoded:02X}").expect("writing to a string succeeds");
            }
            bytes = rest;
        } else if byte.is_ascii() {
            normalized.push(char::from(byte));
        } else {
            write!(normalized, "%{byte:02X}").expect("writing to a string succeeds");
        }
    }
    Cow::Owned(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_wildcards_and_end_anchors_in_request_targets() {
        for (pattern, target, expected) in [
            ("/", "/anything", true),
            ("/*.php$", "/fish.php", true),
            ("/*.php$", "/fish.php?p=1", false),
            ("/*a*b$", "/babb", true),
            ("/*a*b$", "/ba", false),
            ("/**/private*", "/a/private?preview=1", true),
            ("/query?preview=*", "/query?preview=1", true),
            ("/Guide", "/guide", false),
            ("/aa*a$", "/aa", false),
            ("/price$*?", "/price$x?", true),
        ] {
            assert_eq!(
                matches_pattern(pattern, target),
                expected,
                "{pattern} {target}"
            );
        }
    }

    #[test]
    fn normalizes_utf8_and_percent_escapes_without_equating_encoded_slashes() {
        let robots = Robots::parse(
            "\u{feff}User-agent: *\rDisallow: /café\r\nDisallow: /f%69sh%2f\r\nDisallow: /doc?preview=*\r\n",
            "robots.txt".to_owned(),
        );
        let blocked = |path: &str| {
            robots.blocking_line(&Url::parse(&format!("https://example.org{path}")).unwrap())
        };
        assert_eq!(blocked("/caf%c3%a9"), Some(2));
        assert_eq!(blocked("/fish%2f"), Some(3));
        assert_eq!(blocked("/fish/"), None);
        assert_eq!(blocked("/doc?preview=1"), Some(4));
        assert_eq!(blocked("/doc"), None);
    }
}
