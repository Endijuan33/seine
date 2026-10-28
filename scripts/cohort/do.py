#!/usr/bin/env python3
"""DigitalOcean droplet leases for cohort and GPU bring-up runs (same interface as aws.py).

up:   import a task-only SSH key, create one droplet (root login with that key), schedule a
      guest power-off at the lease deadline and a local systemd timer that deletes the
      droplet independently. A powered-off droplet still bills, so the timer and `down`
      are what end the charge.
ip:   print the public IPv4 once active.
down: delete the droplet and the SSH key.

Uses doctl (token in ~/.config/doctl/config.yaml). Receipts: ~/.cache/seine-hashrate/leases/<name>.json.
GPU sizes (doctl hides them from `size list`; the API lists them): gpu-mi300x1-192gb,
gpu-mi325x1-256gb (nyc2/tor1), gpu-h100x1-80gb, ...; the ROCm image slug is
digitaloceanai-rocmsoftware (ROCm 6.4.1 on Ubuntu 24.04).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

CACHE = Path.home() / '.cache/seine-hashrate'
LEASES = CACHE / 'leases'
DOCTL = os.environ.get('DOCTL') or shutil.which('doctl') or str(Path.home() / '.local/bin/doctl')


def doctl(*args, check=True):
    result = subprocess.run([DOCTL, *args, '--output', 'json'], capture_output=True, text=True, timeout=600)
    if result.returncode:
        if check:
            raise SystemExit(f"doctl {' '.join(args[:3])}: {result.stderr.strip()[:600]}")
        return None
    return json.loads(result.stdout) if result.stdout.strip() else None


def receipt_path(name):
    return LEASES / f'{name}.json'


def save(receipt):
    receipt_path(receipt['name']).write_text(json.dumps(receipt, indent=2))


def cmd_up(args):
    LEASES.mkdir(parents=True, exist_ok=True)
    if receipt_path(args.name).exists():
        raise SystemExit(f'lease {args.name} already exists')
    key = CACHE / f'{args.name}-ssh'
    subprocess.run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', args.name, '-f', str(key)], check=True)
    receipt = {'name': args.name, 'provider': 'do', 'region': args.region, 'key': str(key),
               'instance_type': args.size, 'image': args.image, 'hours': args.hours, 'market': 'on-demand'}
    save(receipt)
    unit = f'seine-lease-{args.name}'
    subprocess.run(['systemd-run', '--user', f'--on-active={int(args.hours * 3600)}', f'--unit={unit}',
                    sys.executable, str(Path(__file__).resolve()), 'down', args.name],
                   check=True, capture_output=True)
    try:
        imported = doctl('compute', 'ssh-key', 'import', f'seine-lease-{args.name}',
                         '--public-key-file', str(key.with_suffix('.pub')))
        key_id = (imported[0] if isinstance(imported, list) else imported)['id']
        receipt['ssh_key_id'] = key_id
        save(receipt)
        minutes = max(5, int(args.hours * 60) - 5)
        with tempfile.NamedTemporaryFile('w', suffix='.yaml', delete=False) as user_data:
            user_data.write(f'#cloud-config\nruncmd:\n  - [shutdown, -P, "+{minutes}"]\n')
            user_data_path = user_data.name
        try:
            created = doctl('compute', 'droplet', 'create', args.name, '--region', args.region,
                            '--size', args.size, '--image', args.image, '--ssh-keys', str(key_id),
                            '--user-data-file', user_data_path, '--tag-name', 'seine-cohort')
        finally:
            Path(user_data_path).unlink(missing_ok=True)
        droplet = created[0] if isinstance(created, list) else created
        receipt['instance_id'] = droplet['id']
        save(receipt)
    except SystemExit:
        cmd_down(args)
        raise
    print(json.dumps({'name': args.name, 'instance_id': receipt['instance_id'], 'delete_after_hours': args.hours}))


def cmd_ip(args):
    receipt = json.loads(receipt_path(args.name).read_text())
    deadline = time.time() + args.wait
    while True:
        droplet = doctl('compute', 'droplet', 'get', str(receipt['instance_id']))
        droplet = droplet[0] if isinstance(droplet, list) else droplet
        status = droplet.get('status')
        ip = next((n['ip_address'] for n in droplet.get('networks', {}).get('v4', []) if n.get('type') == 'public'), None)
        if status == 'active' and ip:
            print(ip)
            return
        if status in ('off', 'archive') or time.time() > deadline:
            raise SystemExit(f'not ready: {status}')
        time.sleep(10)


def cmd_down(args):
    path = receipt_path(args.name)
    if not path.exists():
        print(f'no lease {args.name}')
        return
    receipt = json.loads(path.read_text())
    if receipt.get('instance_id'):
        subprocess.run([DOCTL, 'compute', 'droplet', 'delete', str(receipt['instance_id']), '--force'],
                       capture_output=True, text=True)
        for _ in range(30):
            if doctl('compute', 'droplet', 'get', str(receipt['instance_id']), check=False) is None:
                break
            time.sleep(10)
        else:
            raise SystemExit(f"droplet {receipt['instance_id']} not deleted")
    if receipt.get('ssh_key_id'):
        subprocess.run([DOCTL, 'compute', 'ssh-key', 'delete', str(receipt['ssh_key_id']), '--force'],
                       capture_output=True, text=True)
    for suffix in ('', '.pub'):
        Path(receipt['key'] + suffix).unlink(missing_ok=True)
    subprocess.run(['systemctl', '--user', 'stop', f"seine-lease-{args.name}.timer"], capture_output=True)
    path.rename(path.with_suffix('.deleted.json'))
    print(f"deleted {args.name} ({receipt.get('instance_id')})")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='cmd', required=True)
    up = sub.add_parser('up')
    up.add_argument('name')
    up.add_argument('--size', required=True)
    up.add_argument('--region', default='nyc2')
    up.add_argument('--image', default='digitaloceanai-rocmsoftware')
    up.add_argument('--hours', type=float, default=1.5)
    ip = sub.add_parser('ip')
    ip.add_argument('name')
    ip.add_argument('--wait', type=int, default=900)
    down = sub.add_parser('down')
    down.add_argument('name')
    args = parser.parse_args()
    {'up': cmd_up, 'ip': cmd_ip, 'down': cmd_down}[args.cmd](args)


if __name__ == '__main__':
    main()
