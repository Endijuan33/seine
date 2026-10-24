#!/usr/bin/env python3
"""Fixed-path control for the shipping-default binary on a disposable Ice Lake guest."""
import hashlib
import json
from pathlib import Path
import subprocess
from bench import paired

root = Path.cwd()
identity = json.loads(subprocess.check_output(['lscpu', '-J'], text=True))
cpu = {r['field'].rstrip(':'): r['data'] for r in identity['lscpu']}
assert cpu['Vendor ID'] == 'GenuineIntel' and cpu['Model'] == '106'
(root / 'hardware.json').write_text(json.dumps(identity, indent=2) + '\n')
expected = json.loads((root / 'expected-binaries.json').read_text())
for name, digest in expected.items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == digest, name
for kernel in ['avx2', 'avx512']:
    result = paired(root, root / ('control-' + kernel),
                    {'binary': 'baseline', 'kernel': kernel},
                    {'binary': 'seine', 'kernel': kernel},
                    workers=12, pairs=2, seconds=20, rounds=2, warmup=1)
    print(kernel, json.dumps(result), flush=True)
(root / 'VALIDATION_COMPLETE').write_text('Both fixed-path controls completed; inspect deltas before merge.\n')
