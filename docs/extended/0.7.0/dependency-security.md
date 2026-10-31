# PandaCode Extended 0.7.0 — Dependency security resolution

**Ownership:** PandaCode Extended only. The upstream PandaCode fork is not patched by these commits.  
**Candidate:** PR [#86](https://github.com/ginmp8/pandacode-extended/pull/86), source based on upstream PandaCode 0.3.85.  
**Status:** **Resolved in Extended source** for the known npm advisories from the audited lockfiles; final CI/native release acceptance remains distinct.  
**Security test evidence:** [GitHub Actions 41243766188](https://github.com/ginmp8/pandacode-extended/actions/runs/41243766188) (and subsequent CI as the PR evolves).

## Dependency remediation

| Workspace | Dependency | Initial vulnerable | Extended candidate | Fix/validation |
| --- | --- | --- | --- | --- |
| Desktop | `dompurify` | 3.4.15 | **3.4.17** | Updated browser HTML sanitizer; audit clean |
| Desktop | `katex` | 0.16.47 | **0.18.2** | Updated math renderer; audit clean |
| Desktop | `vite` | 7.3.6 | **7.3.7** | Development server advisory fixes; audit clean |
| Desktop + website | `source-map-js` | 1.2.1 | **1.2.2** | Parser denial-of-service fix; audit clean |
| Website | `vite` | 8.3.0 | **8.3.3** | Dev-server update; Rolldown 1.2.12 and native bindings aligned |
| Website | `devalue` | 5.9.2 | **5.9.3** | Serializer hardening; audit clean |
| Website | `sharp` | 0.35.4 | **0.35.5** | Native libvips and WASM bindings aligned; audit clean |
| Website | `smol-toml` | 1.8.0 | **1.9.0** | Parser complexity fix; audit clean |
| Website | `undici` | 8.10.2 | **8.12.2** | HTTP/TLS/DoS fixes; audit clean |
| Website | `zod` | 4.6.5 | **4.6.6** | Validation memory fix; audit clean |
| Mobile (Expo 58) | `glob` | 13.0.6 | **14.0.3** | Compatibility tests and audit |
| Mobile (Expo 58) | `minimatch` | 10.2.6 / 3.1.5 | **11.0.3** | Includes nested ESLint copies; pattern checks and audit |
| Mobile (Expo 58) | `node-forge` | 1.4.0 | **1.4.2** | Cryptographic parsing updates; hash smoke tests |
| Mobile (Expo 58) | `zod` | 3.25.76 | **4.6.7** | Validation tests and audit |
| Mobile (Expo 58) | `path-scurry` | 2.0.2 | **3.0.0** | Required by updated glob |
| Mobile (Expo 58) | `deepmerge` | 4.3.1 | **4.3.2-extended.1** *(local source fork)* | Explicit source and target prototype safety checks |

The original audit also reported several indirect/mobile advisory nodes dependent on these vulnerable packages; those aggregated npm findings are resolved by fixing the dependency graph, not by dropping or ignoring alerts.

## Deepmerge — Extended-local security patch

**Important:** There is **no published upstream secure version of deepmerge** confirmed for the observed prototype-poisoning behavior. Downgrading from 4.3.1 to 4.1.1 made the reported advisory disappear, but an independent test showed the unsafe assignment **still occurred**. Therefore **4.1.1 is not considered a security fix**.

The Extended project now uses [`mobile/vendor/extended/deepmerge`](../../../mobile/vendor/extended/deepmerge/README.md): a source copy of MIT-licensed upstream 4.3.1 with a bounded check rejecting dangerous `__proto__`, `prototype` and `constructor` keys from both merge inputs. The associated helper and license are retained, with an Extended-only internal version and an npm local-file override. There is no claim that this fork was released by upstream.

The unit regression [`mobile/tests/dependency-security.test.ts`](../../../mobile/tests/dependency-security.test.ts) covers inherited prototype integrity and correctly merged ordinary object properties, plus smoke checks for `glob`/`minimatch`, `zod` and `node-forge`. Replacing the vendor fork with an official patched upstream package requires independently verifying those regressions before removing the override.

## CI and evidence

- **Baseline:** The original root audit reported 4 vulnerable packages; the separate website audit reported 5 vulnerable packages. The mobile audit reported **27 affected npm dependency nodes (3 high, 23 moderate, 1 low)**.
- **Dependency audit:** [CI 41243766188](https://github.com/ginmp8/pandacode-extended/actions/runs/41243766188) — `npm audit --audit-level=low` against root, website, mobile and relay returned **0 vulnerabilities for each**.
- **Mobile tests:** The mobile job performs a frozen `npm ci --ignore-scripts`, TypeScript typecheck, unit/security regression tests, `expo config --json`, ESLint and an iOS JavaScript export smoke. A run is only considered accepted when the corresponding job concludes **success**; do not infer iOS native binary compatibility from JS export.
- **Desktop CI:** TypeScript/Node/website, Rust tests/Clippy and ONNX parity provide separate regression gates; a successful audit alone is not their result.
- **Release boundary:** No production installer, native iOS/Android runtime smoke, Rust `cargo audit` or supply-chain provenance verification is claimed from these npm audits.

## Policies and ownership

Security audit checks remain mandatory and failing by default; no `--force` installation, npm audit allowlist, skipped advisory or CI threshold relaxation was used. The Extended documentation lives under `docs/extended/`, its release notes under `releases/extended/`, and its local dependency patch under `mobile/vendor/extended/`. Neither the upstream PandaCode documentation nor its GitHub fork is modified by this Extended-only remediation.

For non-dependency application findings, see [Known issues](known-issues.md). For publication identity and release status, see [Extended 0.7.0 release notes](../../../releases/extended/0.7.0.md).

## Rust dependency security follow-up for Extended 0.7.0

**Scope:** PandaCode Extended only, based on `base/hardened`; no changes to the upstream fork. These are review candidates pending full CI and platform validation.

| Dependency | Previous lock | Candidate lock | Reason |
| --- | --- | --- | --- |
| `rustls` | 0.23.45 | 0.23.46 | RUSTSEC-2026-0346 and 0347 (plus other applicable 0.23.46 fixes) |
| `rustls-webpki` | 0.103.15 | 0.103.16 | RUSTSEC-2026-0382 / CVE-2026-111943 |
| `webpki-roots` (resolved 1.x) | 1.0.9 | 1.0.11 | RUSTSEC-2026-0381 / CVE-2026-111943 |
| `webpki-root-certs` | 1.0.9 | 1.0.11 | RUSTSEC-2026-0383 / CVE-2026-111943 |
| `tokio` | 1.53.1 | 1.53.3 | RUSTSEC-2026-0386 (unsafe I/O descriptor construction) |
| Direct `zip` | 2.4.2 | 9.0.4 | RUSTSEC-2026-0367 through 0369: extraction privilege escalation and symlink safety |
| `zip` via `tauri-plugin-updater` | 4.6.1 | removed from resolved graph | Disable the updater's optional legacy Windows ZIP-extraction feature, preserving `rustls-tls` |

The `webpki-roots` 0.26.11 compatibility wrapper is retained and now references the patched 1.x roots. The direct `zip` upgrade uses its existing restricted `deflate` feature; theme-import code uses `ZipArchive::by_name`, not `ZipArchive::extract`.

**Updater compatibility boundary:** `tauri-plugin-updater` remains at 2.9.0 with its optional `zip` feature disabled. Existing direct signed update transport remains available. Legacy compressed Windows update formats such as `.nsis.zip` and `.msi.zip` are not supported by this configuration until tested. The current Tauri bundle target is `.app`; Windows installers are not release-validated here.

**Supply-chain controls:** `actions/checkout` is pinned to v5.1.0 (SHA `fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09`) and `actions/setup-node` to v5.0.0 (SHA `a0853c24544627f65ddf259abe73b1d18a591444`) in CI and release workflows. The actions use Node 24 internally; application jobs still select Node 22 explicitly. A separate RustSec `cargo-audit` job audits the committed `Cargo.lock` without ignored advisories. npm audits still cover desktop, website, mobile, and relay. An additional Cloudflare Worker audit resolves the standalone manifest on the runner **without a committed lock**, so its results are non-reproducible until a dedicated lockfile is added.

**Validation required before claiming resolution:** `cargo metadata --locked`, `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`, `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo audit --file src-tauri/Cargo.lock`, npm audits, VSIX theme import regressions, TLS connectivity, and platform-specific updater behavior. The Cargo.lock was reconciled against the official crates.io index for version/checksum metadata and checked for missing graph references; **Cargo tooling was unavailable in the local authoring environment**, and this does not replace Cargo resolution/build verification. If CI reports other advisories, do not ignore or classify them as resolved.

Existing Extended security controls and the locally patched `mobile/vendor/extended/deepmerge` fork must remain unchanged.

### Cloudflare Worker npm advisories found by the new CI gate

The first execution of the separate Cloudflare Worker audit in [CI run 41252501026](https://github.com/ginmp8/pandacode-extended/actions/runs/41252501026) reported **5 npm findings (3 high, 2 moderate)** in the previously uncovered nested package installation. These findings were not visible in the four existing workspace npm audits. The native npm audit recommended versions outside the pinned ranges; do not use `npm audit fix --force`.

- `@cloudflare/sandbox` **1.0.1 → 1.0.3** for the affected transitively resolved `zod` advisory [GHSA-mrhx-c7xc-m3w5](https://github.com/advisories/GHSA-mrhx-c7xc-m3w5).
- `wrangler` **4.151.0 → 4.156.0** to leave the vulnerable Wrangler/Miniflare/`undici` dependency range, including [GHSA-2m5v-pch6-74hx](https://github.com/advisories/GHSA-2m5v-pch6-74hx) and [GHSA-4qv8-2p9j-2685](https://github.com/advisories/GHSA-4qv8-2p9j-2685).

The npm audit CI check remains fail-closed. **CI must confirm these new pins actually resolve without advisories.** A committed lockfile for this nested package is still required for fully reproducible installs; the current CI generates an ephemeral lockfile only for scanning.
