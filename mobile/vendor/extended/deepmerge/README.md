# PandaCode Extended: security-maintained deepmerge

This directory is maintained by **PandaCode Extended**. It is not an upstream
PandaCode feature and not a published upstream `deepmerge` version.

## Source and change

- Upstream: [deepmerge 4.3.1](https://github.com/TehShrike/deepmerge/tree/v4.3.1), MIT.
- Embedded helper: [is-mergeable-object 1.1.0](https://github.com/TehShrike/is-mergeable-object/tree/v1.1.0), MIT.
- Internal version: `4.3.2-extended.1` (not published to npm).
- Local installation: `mobile/package.json` uses `overrides.deepmerge` pointing
  to `file:./vendor/extended/deepmerge`.

The original 4.3.1 `mergeObject()` can assign attacker-controlled
`__proto__`, `constructor`, or `prototype` keys to the destination. In this
Extended fork, both the target-copy and source-copy passes ignore these keys
before recursive merge or assignment. The rest of the upstream API is retained,
including array concatenation, customMerge, merge.all and TypeScript types.

## Validation and maintenance

- The test in `mobile/tests/dependency-security.test.ts` checks poisoned
  **source and target**, nested config semantics, and preservation of
  `Object.prototype`. Never remove or weaken that regression.
- `npm audit` is still required. A clean advisory lookup alone is not
  proof that a local vendored dependency is secure.
- Replace this fork with an official patched `deepmerge` version only after
  upstream publishes one, the new version passes both poisoning regressions
  and Expo config/native build tests, and the lockfile no longer requires
  this local override.
