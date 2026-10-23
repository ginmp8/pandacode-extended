# GLib 0.18.5: upstream iterator soundness fix

This directory contains the original `glib` 0.18.5 crate with the two-line
upstream fix for **RUSTSEC-2024-0429**. Its package version remains **0.18.5**.
GTK3-dependent APIs remain compatible. The original `LICENSE`, `COPYRIGHT`,
source files, and crate metadata are retained.

## Source and exact change

- Original archive: <https://static.crates.io/crates/glib/glib-0.18.5.crate>
- Archive SHA-256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`
- Advisory: <https://rustsec.org/advisories/RUSTSEC-2024-0429.html>
- Upstream review: <https://github.com/gtk-rs/gtk-rs-core/pull/1343>
- Upstream fix commit: `b5a4071e439bef2b5eea76c3aa25e5ae84839e34`
- Patched `src/variant_iter.rs` SHA-256: `a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc`
- Stored `BACKPORT.patch` SHA-256: `430e067626717bf08d3e0af470824dbe0a59b232e924be8409c55b2faab80ffc`

The original crate archive was checked against the checksum in Cargo.lock
before extraction. `BACKPORT_PROVENANCE.json` records the original hash of
every upstream file and the expected hash of the sole modified source file.
`BACKPORT.patch` contains the exact source change, with paths relative to this
crate: make the C out-parameter pointer mutable and pass `&mut p` instead of
`&p`. No other upstream file was edited.

PandaCode selects this copy through its `[patch.crates-io]` table. Version-only
scanners may still flag 0.18.5; registry-only scanners may omit a path dependency.
Neither outcome proves that this backport is present. Verify the provenance and
run the optimized regression below. Do not relabel this crate as 0.20.0 or add a
blanket advisory ignore.

## Verification

From `src-tauri`, with Python 3.11+ and Rust 1.88+ (required by ZIP 9):

```sh
python3 vendor/glib/verify-backport.py
cargo test --manifest-path security-tests/Cargo.toml --release --locked
```

The separate test package exercises forward, backward, indexed, and final-item
string iteration with optimization enabled, where the original unsound write
could be discarded. It also checks the ZIP 9 archive APIs used by the VSIX
reader. It requires the normal GLib development libraries but does not build
or launch PandaCode. Its dependencies are locked independently.

When updating the upstream GTK stack, reevaluate this patch and remove it only
after every consumer resolves a fixed upstream GLib version.
