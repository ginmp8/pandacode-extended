# Source fidelity at upstream 0.3.87

- Source: `ginmp8/pandacode` at `4579dd4f6b69219bd9975d6b1aa2c19e76cba7d1` (`chore: release 0.3.87`).
- Target: `ginmp8/pandacode-extended:base/development`, before repair `7c4191ee5ec10e243fcafbdb6eb07284362f1a56`.
- Upstream 0.3.85 -> 0.3.87 introduced changes to 32 distinct tracked paths. Before this repair, all 27 non-version paths matched the pinned release and five version/lock paths remained at 0.3.85.
- This repair synchronizes the application version in `package.json`, `package-lock.json` (root and empty package), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (the `pandacode` package), and `src-tauri/tauri.conf.json`. It leaves all Extended dependency security fixes and update/Connect defaults intact.
- It also restores 57 source-identical historical `docs/app-store/` files (including images) that were missing since the upstream 0.3.85 baseline; original Git blob SHAs are retained. These are ORIGINAL upstream App Store instructions/assets, not authorization to deploy an Extended app or use upstream Apple credentials, publishing identities or domains.
- `source-baseline.json` pins source release identity and original release-note/date hashes. `scripts/release/version-identity.test.mjs` verifies this identity before Release CI builds and as part of `npm run test:release`.
- The remaining 58 pre-existing divergent paths between the fork and Extended must be assessed under their respective security and Extended-only contracts before claiming byte-for-byte tree parity; no risky runtime file or dependency graph is overwritten by this repair.
- Release `0.3.87` is the application/source version, NOT a GitHub Release or release assignment for Extended 0.7.0. `releases/extended/` is unchanged, with no tags, publishing, installer distribution, or PR operations.
- Verification: Node fixture red/green/negative regression was executed in the isolated sandbox. Full repository npm/Rust/UI tests and exact-SHA CI must be evaluated independently; no success is claimed here without those checks.
