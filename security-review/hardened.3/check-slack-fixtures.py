#!/usr/bin/env python3
"""Fail closed on Slack token-shaped values in a source distribution.

Scans data, never executes anything, and never prints candidate secret bytes.
Run: python security-review/hardened.3/check-slack-fixtures.py <repo-root>
"""
from pathlib import Path
import re
import sys

PATTERN = re.compile(rb'(?<![A-Za-z0-9])(?:xoxb|xapp|xoxp|xoxa|xoxs|xoxr)-[A-Za-z0-9-]{16,}')
SKIP = {'.git', 'node_modules', 'target', 'dist', '__pycache__'}


def main() -> int:
    if len(sys.argv) != 2:
        print('usage: check-slack-fixtures.py <repo-root>', file=sys.stderr)
        return 2
    root = Path(sys.argv[1]).resolve(strict=True)
    if not root.is_dir():
        print('Expected a directory', file=sys.stderr)
        return 2
    candidates = []
    inspected = 0
    for path in root.rglob('*'):
        if any(part in SKIP for part in path.relative_to(root).parts):
            continue
        if path.is_symlink():
            print(f'Refusing symlink: {path.relative_to(root)}', file=sys.stderr)
            return 2
        if not path.is_file():
            continue
        try:
            data = path.read_bytes()
        except OSError as exc:
            print(f'Cannot read: {path.relative_to(root)}: {type(exc).__name__}',file=sys.stderr)
            return 2
        inspected += 1
        for match in PATTERN.finditer(data):
            # Never print the token; only non-sensitive source coordinates.
            line = data.count(b'\n', 0, match.start()) + 1
            candidates.append(f'{path.relative_to(root).as_posix()}:{line}')
    if candidates:
        print(f'FAIL: {len(candidates)} Slack-like secret literals in {inspected} files:')
        for label in candidates:
            print('  ' + label)
        return 1
    print(f'PASS: {inspected} files checked; no Slack token-shaped values detected')
    return 0


if __name__ == '__main__':
    sys.exit(main())
