#!/usr/bin/env python3
"""Offline integrity check for the documented upstream GLib backport."""
import hashlib
import json
from pathlib import Path
import tomllib

root = Path(__file__).resolve().parent
provenance = json.loads((root / "BACKPORT_PROVENANCE.json").read_text())
package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
assert package["name"] == "glib" and package["version"] == "0.18.5"
assert provenance["crate_sha256"] == "233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5"
assert provenance["upstream_fix_commit"] == "b5a4071e439bef2b5eea76c3aa25e5ae84839e34"
original = provenance["original_files_sha256"]
patched = provenance["patched_files_sha256"]
assert set(patched) == {"src/variant_iter.rs"}
assert patched["src/variant_iter.rs"] == "a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc"
assert provenance["patch_sha256"] == "430e067626717bf08d3e0af470824dbe0a59b232e924be8409c55b2faab80ffc"
assert hashlib.sha256((root / "BACKPORT.patch").read_bytes()).hexdigest() == provenance["patch_sha256"]
for name, original_hash in original.items():
    path = root / name
    assert path.is_file() and not path.is_symlink(), f"Missing or linked file: {name}"
    expected = patched.get(name, original_hash)
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    assert actual == expected, f"Unexpected contents: {name}"
source_files = {str(path.relative_to(root)) for path in (root / "src").rglob("*") if path.is_file()}
assert source_files == {name for name in original if name.startswith("src/")}
print(f"Verified {len(original)} upstream files: one documented patch; license and version preserved.")
