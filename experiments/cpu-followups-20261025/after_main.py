#!/usr/bin/env python3
"""Wait without CPU load, then run alignment only within the existing lease."""
import argparse
import json
from pathlib import Path
import subprocess
import time


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--deadline', type=float, required=True)
    args = p.parse_args()
    root = Path(__file__).resolve().parent
    while True:
        remaining = args.deadline - time.time()
        if remaining < 1020:
            raise RuntimeError('Insufficient lease time for bounded alignment test')
        state = subprocess.check_output(['systemctl', 'show', 'seine-followups-controller', '--property=ActiveState', '--value'], text=True).strip()
        if state not in ['active', 'activating', 'deactivating']:
            break
        time.sleep(15)
    assert json.loads((root.parent / 'results/outcome.json').read_text())['status'] == 'COMPLETE'
    print('Main controller complete; starting bounded alignment experiment', flush=True)
    subprocess.run(['python3', str(root / 'run_alignment.py')], cwd=root, check=True, timeout=960)


if __name__ == '__main__':
    main()
