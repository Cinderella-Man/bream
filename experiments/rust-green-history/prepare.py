#!/usr/bin/env python3
"""Restore pinned historical inputs into ignored workspace paths; no execution."""
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def main():
    provenance = json.loads((ROOT / 'evidence/rust-green-history/provenance.json').read_text())
    binary = provenance['official_compiler']
    archive = ROOT / binary['archive_path']
    archive.parent.mkdir(parents=True, exist_ok=True)
    data = archive.read_bytes() if archive.exists() else fetch(binary['url'])
    assert digest(data) == binary['archive_sha256']
    if not archive.exists():
        archive.write_bytes(data)
    destination = ROOT / binary['unpacked_path']
    if not destination.exists():
        with tarfile.open(fileobj=io.BytesIO(data)) as tar:
            assert all(Path(m.name).parts[0] == destination.name for m in tar.getmembers())
            tar.extractall(destination.parent, filter='data')
    for snapshot in provenance['source_snapshots']:
        manifest = json.loads((ROOT / snapshot['manifest']).read_text())
        checkout = ROOT / snapshot['checkout']
        if 'archive_url' in manifest:
            if all((checkout / f['path']).exists() and digest((checkout / f['path']).read_bytes()) == f['sha256'] for f in manifest['files']):
                continue
            data = fetch(manifest['archive_url'])
            assert digest(data) == manifest['archive_sha256']
            with tarfile.open(fileobj=io.BytesIO(data)) as tar:
                members = {'/'.join(Path(m.name).parts[1:]): m for m in tar.getmembers() if m.isfile()}
                for item in manifest['files']:
                    blob = tar.extractfile(members[item['path']]).read()
                    assert digest(blob) == item['sha256']
                    target = checkout / item['path']
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(blob)
        else:
            for item in manifest['files']:
                target = checkout / item['path']
                blob = target.read_bytes() if target.exists() else fetch(item['url'])
                assert digest(blob) == item['sha256']
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(blob)
    print('Pinned historical inputs verified; no compiler, installer, or experiment executed.')


if __name__ == '__main__':
    main()
