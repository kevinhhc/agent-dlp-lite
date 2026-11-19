#!/usr/bin/env python3
"""Rebuild notices from fetched locked dependency sources (developer task)."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'], cwd=root))
parts = ['# Third-party notices\n\nThis distribution uses Rust crates under their own licenses. The project license does not replace those licenses. These notices reproduce license/notice files from locked dependency sources, including platform-specific and build dependencies.\n']
count = 0
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['source'] is None:
        continue
    directory = Path(package['manifest_path']).parent
    notices = [p for p in sorted(directory.iterdir()) if p.is_file() and p.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE'))]
    if not notices and (directory / 'AUTHORS').is_file():
        # Some crates publish their licensing statement and author grants here.
        notices = [directory / 'AUTHORS']
        if (directory / 'README.md').is_file():
            notices.append(directory / 'README.md')
    if not notices:
        raise SystemExit('Missing root license notice: ' + package['name'])
    parts.append('\n## ' + package['name'] + ' ' + package['version'] + '\n\nDeclared license: ' + str(package.get('license')) + '\n\n' + '\n\n'.join(p.name + '\n\n' + p.read_text(errors='replace') for p in notices))
    count += 1
(root / 'THIRD_PARTY_NOTICES.md').write_text('\n'.join(parts))
print('Wrote notices for', count, 'locked dependencies')
