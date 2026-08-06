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

static CLASH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)eaddrinuse|already in use").expect("clash regex"));

static CLASH_PORT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)port\s+(\d{1,5})|:(\d{1,5})\b").expect("clash port regex"));

/// Does this line complain that the doorstep is occupied?
pub fn is_port_clash(line: &str) -> bool {
    CLASH.is_match(line)
}

/// Which port a clash complaint names, if any (vite: "Port 1420 is already
/// in use"; node: "EADDRINUSE ... :::3000").
pub fn clash_port(line: &str) -> Option<u16> {
    if !is_port_clash(line) {
        return None;
    }
    let mut last = None;
    for cap in CLASH_PORT.captures_iter(line) {
        if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
            if let Ok(p) = m.as_str().parse::<u16>() {
                last = Some(p);
            }
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clash_lines_are_detected_and_ported() {
        assert!(is_port_clash("Error: Port 1420 is already in use"));
        assert_eq!(clash_port("Error: Port 1420 is already in use"), Some(1420));

        let node = "Error: listen EADDRINUSE: address already in use :::3000";
        assert!(is_port_clash(node));
        assert_eq!(clash_port(node), Some(3000));

        let node4 = "Error: listen EADDRINUSE: address already in use 127.0.0.1:5173";
        assert_eq!(clash_port(node4), Some(5173));

        assert!(!is_port_clash("  VITE v6.3.5  ready in 320 ms"));
        assert!(!is_port_clash("Local:   http://localhost:5173/"));
    }
}
