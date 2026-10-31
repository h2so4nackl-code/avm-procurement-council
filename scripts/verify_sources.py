#!/usr/bin/env python3
"""Ensure vendored upstream code is byte-identical to the pinned snapshot."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
manifest=json.loads((ROOT/'UPSTREAM.json').read_text())
for path,expected in manifest['unmodified_files_sha256'].items():
    assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==expected,path
print('Pinned upstream source bytes: PASS')
