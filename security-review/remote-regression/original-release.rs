type Result<T> = std::result::Result<T, String>;
use std::collections::HashMap;
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
        if !(16..=64).contains(&nonce.len()) || !nonce.chars().all(|c| c.is_ascii_graphic()) || (now_ms - ts).abs() > WINDOW_MS {
            return Err(REFUSED.into());
        }
        // An accepted nonce only needs remembering while its ts is still inside the window.
        self.seen.retain(|_, seen| *seen >= now_ms - WINDOW_MS);
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
        let now = 1_000_000_000_i64;
        for timestamp in [i64::MIN, i64::MAX, now.wrapping_add(i64::MIN)] {
            assert!(replay.check(Some(timestamp), Some(nonce), now).is_err());
        }
        assert!(replay.check(Some(i64::MIN), Some(nonce), i64::MIN).is_ok());
        assert!(replay.check(Some(i64::MAX), Some("0123456789abcdef4567"), i64::MAX).is_ok());
    }


}
