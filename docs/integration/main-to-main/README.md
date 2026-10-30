# Main-to-main upstream integration — experimental

## Authority / immutable source identities

- Extended destination: `ginmp8/pandacode-extended:main@f0105e7db85a0d7bc2e70726716fcfa939fac182` (0.6.0 candidate).
- Full source: `ginmp8/pandacode:main@0336a2fb5e3abaea35c095fcc63805652f49ea98` (contains upstream source through release 0.3.79 and newer documentation).
- Common **content** ancestor for a synthetic three-way merge: original PandaCode `0.3.28@70bdc708303c8681d8eb0b46f2cdb0ff087aa6eb`. This **does not** fabricate Git ancestry between the two independent repositories.
- Branch: `integration/0.7.0-main-to-main`, created from **Extended main**, without incorporating `development`.
- Existing earlier branch `integration/0.7.0-upstream-0.3.79` and its PR #77 are unrelated and will not be merged or reused.

## Objective and hard boundaries

Bring **every upstream main feature** forward, preserving **Extended-specific features** and hardening; all relevant upstream production sources, generated integration contracts and test coverage must be accounted for. The comparison is source+behavior based, not release-note or issue-count based. No hidden data egress, updater, silent installation, unapproved remote Full, permission broadening, changed local storage migration or removal of Board/GitHub/Extended-only functionality may be silently accepted.

The original source 0.3.28 is used only as a merge ancestor for structural comparison; conflict-free merging **does not** imply secure behavior or full feature parity. Reconcile every conflict from both sides, and review upstream-only additions that touch security/permissions/network/provisioning even when Git does not flag a conflict.

## Assessment and acceptance

`.github/workflows/experimental-upstream-merge-assessment.yml` produces a read-only GitHub Actions artifact with source path deltas and a 3-way merge conflict inventory. It does not modify the working application or push generated source.

After conflicts are classified, apply the merged candidate in small validated checkpoints on this branch. Require build, Node/unit, frontend/browser, Rust, mobile/relay, regression and security checks before proposing merge; native Windows and live remote device/authorization behavior still need platform evidence. Treat unverified checks as blockers. Never close existing 0.7.0/0.8.0 issues solely because source files were copied. No release or merge to main without review.
