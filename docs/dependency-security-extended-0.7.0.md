# PandaCode Extended 0.7.0 — Dependency security remediation

**Scope:** PR [#86](https://github.com/ginmp8/pandacode-extended/pull/86); upstream source import plus targeted npm dependency patches.  
**Candidate version:** Extended `0.7.0` planned, not tagged or published.  
**Code baseline:** [`73bb3f29`](https://github.com/ginmp8/pandacode-extended/commit/73bb3f29dcef48f1675651e5be346c06f3600a56).  
**Status:** Desktop and website patches applied; mobile/Expo audit remains unresolved. Do **not** describe the overall CI as green or release-ready.

## Remediated npm dependencies

| Workspace | Package | Before | Patched | Why |
| --- | --- | --- | --- | --- |
| Desktop root | `dompurify` | 3.4.15 | **3.4.17** | Runtime HTML/SVG sanitizer, critical advisory |
| Desktop root | `katex` | 0.16.47 | **0.18.2** | Conditional trust-bypass advisory |
| Desktop root | `vite` | 7.3.6 | **7.3.7** | Dev-server/editor middleware advisories |
| Desktop + website | `source-map-js` | 1.2.1 | **1.2.2** | Indexed source-map denial of service |
| Website | `vite` | 8.3.0 | **8.3.3** | Dev-server/editor middleware advisories |
| Website | `rolldown` | 1.2.9 | **1.2.12** | Required by patched Vite; all optional platform bindings updated |
| Website | `@oxc-project/types` | 0.150.0 | **0.152.0** | Exact Rolldown dependency alignment |
| Website | `devalue` | 5.9.2 | **5.9.3** | Serialization and resource-use advisories |
| Website | `sharp` | 0.35.4 | **0.35.5** | Fixed librsvg dependency; native Sharp, libvips and WASM bindings aligned |
| Website | `smol-toml` | 1.8.0 | **1.9.0** | Parser CPU amplification |
| Website | `undici` | 8.10.2 | **8.12.2** | HTTP, TLS, WebSocket and resource-use advisories |
| Website | `zod` | 4.6.5 | **4.6.6** | Oversized collections memory-exhaustion advisory |

Patched versions and npm SHA-512 integrity fields were cross-checked against public package-lock records and official GitHub advisories. These updates do **not** run `npm audit fix --force` and do not suppress security checks. The root `package.json` raises the direct dependency minimum for DOMPurify, KaTeX and Vite; the website changes are compatible transitive lockfile updates.

## Executed validation

- **Root `npm audit`:** `found 0 vulnerabilities` in GitHub Actions [run 41241914440](https://github.com/ginmp8/pandacode-extended/actions/runs/41241914440).
- **Website `npm audit`:** `found 0 vulnerabilities` in the same run following the five additional website dependency updates.
- **Website `npm ci`:** the initial 0.35.5 Sharp patch required an additional `@img/sharp-wasm32` binding adjustment; that was applied in [`73bb3f29`](https://github.com/ginmp8/pandacode-extended/commit/73bb3f29dcef48f1675651e5be346c06f3600a56), after which the web CI advanced to build and tests.
- **Latest CI:** [run 41242063265](https://github.com/ginmp8/pandacode-extended/actions/runs/41242063265) includes desktop/website audits and app/website builds. Its complete result should be checked before accepting this candidate.
- **Not executed locally:** the container cannot resolve the npm registry, so network-dependent tests were run by GitHub Actions. No native desktop smoke test, cross-platform installer test or comprehensive Rust dependency vulnerability scan is claimed.

## Unresolved dependency security: mobile / Expo

The newly added mobile lockfile audit reported **27 vulnerabilities (3 high, 23 moderate, 1 low)** in the imported Expo/React Native dependency graph. Examples include:

- `node-forge` certificate/TLS, parser and cryptographic-validation advisories;
- `zod` array-validation resource exhaustion;
- `glob` and nested `minimatch` versions used across Expo/Metro and ESLint tooling; 
- `deepmerge` prototype poisoning under the audited range.

The audit reports some problems as fixable by ordinary update, but others have no compatible fix within existing transitive constraints; `npm audit fix --force` proposes an **Expo 51 downgrade**, which would break the current **Expo 58** dependency contract. This cannot be accepted as a security remediation without a separate compatibility migration and native/mobile validation.

**Required follow-up:** inspect the mobile dependency graph, pin supported security-fix releases where compatible, then coordinate upstream Expo/Metro and ESLint changes for the incompatible paths. Validate mobile `npm ci`, native build, bundled runtime boundaries and a clean mobile audit. See [known issue EXT-070-KI-06](known-issues-extended-0.7.0.md#ext-070-ki-06--mobile-expo-dependency-audit-remains-open).

The relay audit step now uses GitHub Actions `if: always()` so it can run even when a previous workspace audit fails. Its outcome on the final commit still requires verification. No audit exception or relaxation is applied.

## Continuous integration gate

The PR now contains a dedicated `Dependency audit (npm lockfiles)` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), which runs the equivalent of:

```sh
npm audit --audit-level=low
npm audit --prefix website --audit-level=low
npm audit --prefix mobile --audit-level=low
npm audit --prefix relay --audit-level=low
```

A nonzero result fails the audit job. Builds and TypeScript/Rust tests are **additional**, not substitutes for audit and reachability review. The full audit gate is currently blocked by mobile findings; do not claim 0.7.0 is publication-ready without an explicit, verified resolution.

## Advisory references

- [DOMPurify critical template/shadow-root issue](https://github.com/advisories/GHSA-cg2g-xh64-v4q7)
- [Vite cross-origin middleware issue](https://github.com/advisories/GHSA-9jrq-w75r-8gcw)
- [source-map-js indexed-map denial of service](https://github.com/advisories/GHSA-68fv-2mgg-jv7q)
- [KaTeX trust bypass after prototype pollution](https://github.com/advisories/GHSA-238p-pmpm-9mq7)
- [Sharp bundled librsvg issue](https://github.com/advisories/GHSA-wq5f-xc86-pv6w)
- [smol-toml parsing DoS](https://github.com/advisories/GHSA-r4xh-jqrq-34v2)
- [undici TLS option validation](https://github.com/advisories/GHSA-2m5v-pch6-74hx)
- [Zod oversized collection DoS](https://github.com/advisories/GHSA-mrhx-c7xc-m3w5)

Document release security outcomes against a **specific immutable commit and CI run**, and do not confuse a clean known-advisory database result with proof against malicious new dependencies.
