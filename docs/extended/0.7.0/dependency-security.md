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

Security audit checks remain mandatory and failing by default; no `--force` installation, npm audit allowlist, skipped advisory or CI threshold relaxation was used. The Extended documentation lives under `docs/extended/`, its release notes under `release/extended/`, and its local dependency patch under `mobile/vendor/extended/`. Neither the upstream PandaCode documentation nor its GitHub fork is modified by this Extended-only remediation.

For non-dependency application findings, see [Known issues](known-issues.md). For publication identity and release status, see [Extended 0.7.0 release notes](../../../release/extended/0.7.0.md).
