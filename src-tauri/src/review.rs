//! Review & Coordination admission gate. Intentionally NO harness launch, no MCP, no
//! writable path, and no inference before host-level confinement is proven.
//! Manual team presets use the existing executor and must be labelled non-isolated.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewReadiness {
    pub ready: bool,
    pub status: &'static str,
    pub proven_providers: Vec<&'static str>,
    pub note: &'static str,
}

/// The current process job/group only handles termination. It does not stop
/// filesystem writes, network escape or inherited credentials. Do not infer
/// capabilities from installed CLI/model catalogs, flags or plan prompts.
#[tauri::command]
pub(crate) fn review_readiness() -> ReviewReadiness {
    ReviewReadiness {
        ready: false,
        status: "containment_unproven",
        proven_providers: Vec::new(),
        note: "No verified filesystem, network and credential confinement for the CLI process tree. Only explicitly triggered manual review is available.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_advertises_isolation_without_os_proof() {
        let status = review_readiness();
        assert!(!status.ready);
        assert_eq!(status.status, "containment_unproven");
        assert!(status.proven_providers.is_empty());
    }
}
