type Result<T> = std::result::Result<T, String>;
mod http {
use std::collections::HashMap;
const MAX_HEAD: usize = 16 * 1024;
#[derive(Debug, Default)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_ascii_lowercase()).map(String::as_str)
    }

    pub(crate) fn bearer(&self) -> Option<&str> {
        self.header("authorization").and_then(|value| value.strip_prefix("Bearer ")).map(str::trim)
    }

    /// True when the browser addressed this computer, not a rebound domain name.
    pub(crate) fn loopback_host(&self) -> bool {
        let host = self.header("host").unwrap_or_default();
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        matches!(name, "127.0.0.1" | "localhost" | "[::1]")
    }
}

fn decode(component: &str) -> String {
    let bytes = component.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    i += 2;
                }
                None => out.push(b'%'),
            },
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parses a request head; `None` when it is malformed.
pub(crate) fn parse_head(head: &str) -> Option<Request> {
    if head.len() > MAX_HEAD {
        return None;
    }
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let target = first.next()?;
    if !matches!(first.next()?, "HTTP/1.0" | "HTTP/1.1") || first.next().is_some()
        || method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_uppercase())
        || !target.starts_with('/') || target.starts_with("//") || target.chars().any(char::is_control)
    {
        return None;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query.split('&').filter(|pair| !pair.is_empty()).map(|pair| pair.split_once('=').unwrap_or((pair, ""))).map(|(k, v)| (decode(k), decode(v))).collect();
    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':')?;
        if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.chars().any(|character| character.is_control() && character != '\t')
            || headers.insert(name.to_ascii_lowercase(), value.trim().to_owned()).is_some()
        {
            return None;
        }
    }
    // Only explicit Content-Length framing is supported by this small server.
    if !headers.contains_key("host") || headers.contains_key("transfer-encoding") || headers.contains_key("expect") {
        return None;
    }
    if headers.get("content-length").is_some_and(|value| value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())) {
        return None;
    }
    Some(Request { method, path: path.to_owned(), query, headers, body: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_http_framing_and_duplicate_authorization_are_rejected() {
        for headers in [
            "Host: localhost\r\nHost: attacker.example",
            "Host: localhost\r\nAuthorization: Bearer first\r\nauthorization: Bearer second",
            "Host: localhost\r\nContent-Length: 1\r\nContent-Length: 2",
            "Host: localhost\r\nTransfer-Encoding: chunked",
            "Host: localhost\r\nExpect: 100-continue",
            "Host : localhost",
            "Host: localhost\r\nX-Test: line\nInjected: value",
        ] {
            assert!(parse_head(&format!("POST /api/chat/id/messages HTTP/1.1\r\n{headers}")).is_none(), "{headers}");
        }
        assert!(parse_head("GET / HTTP/1.1 extra\r\nHost: localhost").is_none());
        assert!(parse_head("GET https://attacker.example/ HTTP/1.1\r\nHost: localhost").is_none());
        assert!(parse_head("GET / HTTP/1.2\r\nHost: localhost").is_none());
        assert!(parse_head(&format!("GET / HTTP/1.1\r\nHost: localhost\r\nX-Large: {}", "a".repeat(MAX_HEAD))).is_none());
        let request = parse_head("POST /api/chat/id/messages HTTP/1.1\r\nHost: 127.0.0.1:47321\r\nAuthorization: Bearer test\r\nContent-Type: application/json\r\nContent-Length: 2").unwrap();
        assert!(request.loopback_host());
        assert_eq!(request.bearer(), Some("test"));
    }
}

}
mod replay {
use std::collections::HashMap;
use crate::Result;
const WINDOW_MS: i64 = 60_000;
const MAX_SEEN: usize = 20_000;
#[derive(Default)]
pub(crate) struct Replay {
    seen: HashMap<String, i64>,
}

impl Replay {
    pub(crate) fn check(&mut self, ts: Option<i64>, nonce: Option<&str>, now_ms: i64) -> Result<()> {
        const REFUSED: &str = "This request expired or was already received. Check that both computers' clocks are right.";
        let (Some(ts), Some(nonce)) = (ts, nonce) else { return Err(REFUSED.into()) };
        if !(16..=64).contains(&nonce.len()) || !nonce.chars().all(|c| c.is_ascii_graphic()) || now_ms.abs_diff(ts) > WINDOW_MS as u64 {
            return Err(REFUSED.into());
        }
        // An accepted nonce only needs remembering while its ts is still inside the window.
        self.seen.retain(|_, seen| *seen >= now_ms.saturating_sub(WINDOW_MS));
        if self.seen.contains_key(nonce) {
            return Err(REFUSED.into());
        }
        if self.seen.len() >= MAX_SEEN {
            return Err("This computer is busy. Try again in a moment.".into());
        }
        self.seen.insert(nonce.to_owned(), ts);
        Ok(())
    }
}


mod tests {
use super::*;
    #[test]
    fn extreme_timestamps_cannot_overflow_the_replay_window() {
        let mut replay = Replay::default();
        let nonce = "0123456789abcdef0123";
        for timestamp in [i64::MIN, i64::MAX] {
            assert!(replay.check(Some(timestamp), Some(nonce), 1_000_000_000).is_err());
        }
        assert!(replay.check(Some(i64::MIN), Some(nonce), i64::MIN).is_ok());
        assert!(replay.check(Some(i64::MAX), Some("0123456789abcdef4567"), i64::MAX).is_ok());
    }


}
}
