#!/usr/bin/env python3
"""Conservative validation for parameter_registry_seed.yaml.

This checks structure and duplicate live bindings. It does not enable parameters and
must not be treated as protocol verification.
"""
from __future__ import annotations
import sys
from collections import defaultdict
from pathlib import Path
import yaml

path = Path(sys.argv[1] if len(sys.argv) > 1 else 'protocol/parameter_registry_seed.yaml')
data = yaml.safe_load(path.read_text(encoding='utf-8'))
errors = []
ids = set()
bindings = defaultdict(list)
for p in data.get('parameters', []):
    pid = p.get('id')
    if not pid or pid in ids:
        errors.append(f'duplicate/missing id: {pid!r}')
    ids.add(pid)
    b = p.get('binding', {})
    kind = b.get('kind')
    if kind == 'cc': key = ('cc', b.get('controller'))
    elif kind == 'cc_pair': key = ('cc_pair', tuple(b.get('controllers', [])))
    elif kind == 'nrpn': key = ('nrpn', b.get('msb'), b.get('lsb'))
    else: key = None
    if key: bindings[key].append(pid)
    gates = p.get('gates', {})
    if gates.get('live_write_enabled') is not False:
        errors.append(f'{pid}: seed must not have live writes enabled')
for key, pids in bindings.items():
    if len(pids) > 1:
        errors.append(f'duplicate binding {key}: {pids}')
if errors:
    print('\n'.join('ERROR: ' + e for e in errors))
    raise SystemExit(1)
print(f'OK: {len(ids)} parameters; {len(bindings)} unique mapped bindings; all live writes disabled.')
