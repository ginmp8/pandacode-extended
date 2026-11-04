# PandaCode Extended 0.9.0 — release readiness

**Status:** development candidate only; no publication, tag, installer, npm deployment or release approval.

## Source and documentation
- [x] Pin upstream `ginmp8/pandacode:main` at [`5ab09c5a`](https://github.com/ginmp8/pandacode/commit/5ab09c5a4da4a819b30b20771ef768866fa4ef56), declared application version `0.3.94`.
- [x] Inventory both Git trees at source `7a66652a` and Extended [`e069ba4b`](https://github.com/ginmp8/pandacode-extended/commit/e069ba4bcf7411c1d9aab6f69c763a228141c29e) / tree `5e2e87bf`: both `truncated=false`, 1,496 source blobs, zero missing, 1,431 identical, 65 differing, 30 Extended-only.
- [x] Read application-version fields from the five root/Tauri manifests and lockfiles on both exact snapshots: all `0.3.94`.
- [x] Create the separate 0.9.0 documentation set without changing upstream release notes or application version fields.
- [ ] Justify each divergent product-code hunk and prove behavior parity in allowed paths, not merely full file-path presence.

## Validation evidence
- [x] [Push CI #41828235015](https://github.com/ginmp8/pandacode-extended/actions/runs/41828235015) completed success with 6/6 checks on exact Extended SHA `e069ba4b`.
- [x] [PR CI #41828349624](https://github.com/ginmp8/pandacode-extended/actions/runs/41828349624) completed success with 6/6 checks on the same SHA (typecheck/unit/website, Linux Rust, mobile/Expo, npm audit, RustSec, ONNX/macOS).
- [ ] Verify CI and required release/build checks for the **final 0.9.0 candidate SHA**; previous successful CI cannot be applied to this documentation commit or future code.
- [ ] Run relevant local focused regression tests and verify the native Windows, macOS and Linux application/installer paths.
- [ ] Exercise complete Extended Settings On/Off, manual/automatic, new/existing process and restart cases, including real two-device Connect, bots, MCP and cloud/VM/credential flows.
- [ ] Review known-issues and dependency-security evidence at the final SHA; validate migration from existing user data.
- [ ] Verify distribution version/identity and approval workflow separately from the inherited upstream application version.
- [ ] Obtain explicit release approval before any publication.

**Limits:** Git checkout was unavailable due to DNS resolution of `github.com` in the current sandbox; no local product test or real-device control acceptance was run. Recursive tree and exact-SHA CI evidence were obtained from authenticated GitHub. These checklist facts describe the `e069ba4b` pre-documentation baseline, not a passing result for the new commit.
