//! Permission decisions shared by remote RPC admission and its execution recheck.
//! Pure policy: device identity, replay and persistence stay in their existing modules.

pub(crate) const FULL_ACCESS_OFF: &str = "Full access is disabled for remote tasks on this computer. Enable it locally in Settings → Remote access, or choose an approval mode.";

pub(crate) fn allow_mode(allow_full_access: bool, mode: &str) -> Result<(), String> {
    match mode {
        "ask" | "auto" | "plan" => Ok(()),
        "full" if allow_full_access => Ok(()),
        "full" => Err(FULL_ACCESS_OFF.into()),
        _ => Err("Unknown permission mode.".into()),
    }
}

/// A missing mode always keeps the hardened approval default. The remote flag
/// authorizes an explicit choice; turning it on never changes a task implicitly.
pub(crate) fn start_mode(requested: Option<&str>, allow_full_access: bool) -> Result<&str, String> {
    let mode = requested.unwrap_or("ask");
    allow_mode(allow_full_access, mode)?;
    Ok(mode)
}

pub(crate) fn configure_mode<'a>(stored: &'a str, requested: Option<&'a str>, allow_full_access: bool) -> Result<&'a str, String> {
    let mode = requested.unwrap_or(stored);
    allow_mode(allow_full_access, mode)?;
    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_access_is_an_independent_explicit_grant() {
        for mode in ["ask", "auto", "plan"] {
            assert!(allow_mode(false, mode).is_ok());
            assert!(allow_mode(true, mode).is_ok());
        }
        assert_eq!(allow_mode(false, "full").unwrap_err(), FULL_ACCESS_OFF);
        assert!(allow_mode(true, "full").is_ok());
        assert!(allow_mode(true, "unknown").is_err());
        assert!(allow_mode(false, "").is_err());
    }

    #[test]
    fn omitted_start_mode_remains_approving_with_or_without_the_grant() {
        assert_eq!(start_mode(None, false).unwrap(), "ask");
        assert_eq!(start_mode(None, true).unwrap(), "ask");
        assert!(start_mode(Some("full"), false).is_err());
        assert_eq!(start_mode(Some("full"), true).unwrap(), "full");
    }

    #[test]
    fn a_full_task_requires_grant_even_for_a_model_only_change() {
        assert!(configure_mode("full", None, false).is_err());
        assert!(configure_mode("ask", Some("full"), false).is_err());
        assert_eq!(configure_mode("full", Some("ask"), false).unwrap(), "ask");
        assert_eq!(configure_mode("full", None, true).unwrap(), "full");
        assert!(configure_mode("ask", Some("unknown"), true).is_err());
    }
}
