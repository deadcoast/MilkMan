//! The porter watches process output for the doorstep address.

use once_cell::sync::Lazy;
use regex::Regex;

static URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(?:localhost|127\.0\.0\.1|0\.0\.0\.0)(?::\d+)?(?:/[^\s\x22'\)\]]*)?")
        .expect("url regex")
});

/// Find the first local URL in a line of output, normalized to localhost.
pub fn sniff(line: &str) -> Option<String> {
    let m = URL.find(line)?;
    let url = m
        .as_str()
        .replace("0.0.0.0", "localhost")
        .replace("127.0.0.1", "localhost");
    Some(url.trim_end_matches('/').to_string())
}
