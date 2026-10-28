#!/usr/bin/env python3
"""Google Compute Engine leases for cohort benchmarks (same interface as aws.py/cherry.py).

up:    create one instance (bare metal "-metal" types or VMs) whose startup script
       installs a task-only SSH key for root and schedules a guest shutdown at the
       lease deadline; the instance also carries a provider-side max run duration
       that deletes it, and a local systemd timer deletes it independently.
ip:    print the public IPv4 once running.
down:  delete the instance.
zones: list the zones offering a machine type.
quota: show a region's CPU quotas (C4D bare metal needs C4D_CPUS >= 384).

Uses the gcloud CLI ($GCLOUD, else `gcloud` on PATH, else ~/google-cloud-sdk/bin/gcloud)
with $SEINE_GCP_PROJECT (default: the active gcloud project) in $SEINE_GCP_ZONE
(default us-east4-b). Receipts: ~/.cache/seine-hashrate/leases/<name>.json.
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
GCLOUD = os.environ.get('GCLOUD') or shutil.which('gcloud') or str(Path.home() / 'google-cloud-sdk/bin/gcloud')
PROJECT = os.environ.get('SEINE_GCP_PROJECT', '')
ZONE = os.environ.get('SEINE_GCP_ZONE', 'us-east4-b')
IMAGE_FAMILY = os.environ.get('SEINE_GCP_IMAGE_FAMILY', 'ubuntu-2404-lts-amd64')
IMAGE_PROJECT = os.environ.get('SEINE_GCP_IMAGE_PROJECT', 'ubuntu-os-cloud')


def gcloud(*args, check=True):
    cmd = [GCLOUD, '--quiet', '--format=json', *args]
    if PROJECT:
        cmd += ['--project', PROJECT]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if result.returncode:
        if check:
            raise SystemExit(f"gcloud {' '.join(args[:3])}: {result.stderr.strip()[:800]}")
        return None
    return json.loads(result.stdout) if result.stdout.strip() else None


def receipt_path(name):
    return LEASES / f'{name}.json'


def save(receipt):
    receipt_path(receipt['name']).write_text(json.dumps(receipt, indent=2))


def startup_script(public_key, minutes):
    # Runs as root on every boot. The task key goes straight into root's
    # authorized_keys (cloud-init's disable_root only rewrites keys it manages),
    # a first-match drop-in keeps root key logins allowed, and the guest powers
    # off at the deadline whatever the provider-side timer does.
    return f"""#!/bin/bash
set -eu
mkdir -p /root/.ssh && chmod 700 /root/.ssh
grep -qF '{public_key}' /root/.ssh/authorized_keys 2>/dev/null || echo '{public_key}' >> /root/.ssh/authorized_keys
chmod 600 /root/.ssh/authorized_keys
mkdir -p /etc/ssh/sshd_config.d
printf 'PermitRootLogin prohibit-password\\n' > /etc/ssh/sshd_config.d/00-seine-lease.conf
systemctl reload ssh 2>/dev/null || systemctl reload sshd 2>/dev/null || true
shutdown -P +{minutes} 'seine lease deadline' || true
"""


def cmd_up(args):
    LEASES.mkdir(parents=True, exist_ok=True)
    if receipt_path(args.name).exists():
        raise SystemExit(f'lease {args.name} already exists')
    key = CACHE / f'{args.name}-ssh'
    subprocess.run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', args.name, '-f', str(key)], check=True)
    metal = args.machine_type.endswith('-metal')
    receipt = {'name': args.name, 'provider': 'gcp', 'project': PROJECT or None, 'zone': args.zone,
               'key': str(key), 'instance_type': args.machine_type, 'hours': args.hours,
               'market': 'spot' if args.spot else 'on-demand', 'metal': metal}
    save(receipt)
    unit = f'seine-lease-{args.name}'
    subprocess.run(['systemd-run', '--user', f'--on-active={int(args.hours * 3600)}', f'--unit={unit}',
                    sys.executable, str(Path(__file__).resolve()), 'down', args.name],
                   check=True, capture_output=True)
    minutes = max(5, int(args.hours * 60) - 5)
    public_key = key.with_suffix('.pub').read_text().strip()
    with tempfile.NamedTemporaryFile('w', suffix='.sh', delete=False) as script:
        script.write(startup_script(public_key, minutes))
        script_path = script.name
    create = ['compute', 'instances', 'create', args.name, '--zone', args.zone,
              '--machine-type', args.machine_type,
              '--image-family', args.image_family, '--image-project', args.image_project,
              '--boot-disk-size', f'{args.disk_gb}GB',
              '--boot-disk-type', 'hyperdisk-balanced' if metal else 'pd-balanced',
              '--maintenance-policy', 'TERMINATE', '--no-restart-on-failure',
              '--metadata-from-file', f'startup-script={script_path}',
              '--no-service-account', '--no-scopes',
              '--labels', f'project=seine-cohort,lease={args.name}']
    if metal:
        create += ['--network-interface', 'nic-type=IDPF']
    if args.spot:
        create += ['--provisioning-model', 'SPOT', '--instance-termination-action', 'DELETE']
    # Provider-side backstop: delete the instance when the lease elapses even if
    # this machine is gone. Dropped (with a note) if the machine type refuses it.
    duration = ['--max-run-duration', f'{int(args.hours * 3600)}s', '--instance-termination-action', 'DELETE']
    try:
        try:
            instance = gcloud(*create, *duration)
        except SystemExit as err:
            if 'run-duration' not in str(err).lower() and 'termination-action' not in str(err).lower():
                raise
            print(f'note: max run duration refused ({str(err)[:160]}); relying on guest shutdown + local timer',
                  file=sys.stderr)
            receipt['max_run_duration'] = False
            instance = gcloud(*create)
    except SystemExit:
        cmd_down(args)
        raise
    finally:
        Path(script_path).unlink(missing_ok=True)
    created = instance[0] if isinstance(instance, list) else instance
    receipt['instance_id'] = created.get('id')
    receipt['self_link'] = created.get('selfLink')
    save(receipt)
    print(json.dumps({'name': args.name, 'instance_id': receipt['instance_id'], 'zone': args.zone,
                      'delete_after_hours': args.hours}))


def describe(receipt):
    return gcloud('compute', 'instances', 'describe', receipt['name'], '--zone', receipt['zone'], check=False)


def cmd_ip(args):
    receipt = json.loads(receipt_path(args.name).read_text())
    deadline = time.time() + args.wait
    while True:
        instance = describe(receipt) or {}
        status = instance.get('status', 'MISSING')
        nics = instance.get('networkInterfaces') or [{}]
        ip = ((nics[0].get('accessConfigs') or [{}])[0]).get('natIP')
        if status == 'RUNNING' and ip:
            print(ip)
            return
        if status in ('STOPPING', 'TERMINATED', 'MISSING') or time.time() > deadline:
            raise SystemExit(f'not ready: {status}')
        time.sleep(10)


def cmd_down(args):
    path = receipt_path(args.name)
    if not path.exists():
        print(f'no lease {args.name}')
        return
    receipt = json.loads(path.read_text())
    if describe(receipt):
        gcloud('compute', 'instances', 'delete', receipt['name'], '--zone', receipt['zone'])
        for _ in range(60):
            if not describe(receipt):
                break
            time.sleep(10)
        else:
            raise SystemExit(f"instance {receipt['name']} not deleted")
    for suffix in ('', '.pub'):
        Path(receipt['key'] + suffix).unlink(missing_ok=True)
    subprocess.run(['systemctl', '--user', 'stop', f"seine-lease-{args.name}.timer"], capture_output=True)
    path.rename(path.with_suffix('.deleted.json'))
    print(f"deleted {args.name} ({receipt.get('instance_id')})")


def cmd_zones(args):
    rows = gcloud('compute', 'machine-types', 'list', '--filter', f'name={args.machine_type}') or []
    for row in rows:
        print(f"{row['zone']}\t{row['name']}\t{row['guestCpus']} vCPU\t{row['memoryMb'] // 1024} GiB")
    if not rows:
        print(f'no zones offer {args.machine_type}')


def cmd_quota(args):
    region = gcloud('compute', 'regions', 'describe', args.region)
    for quota in region.get('quotas', []):
        if 'CPUS' in quota['metric'] or quota['metric'] in ('INSTANCES',):
            print(f"{quota['metric']}\t{quota['usage']:.0f}/{quota['limit']:.0f}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='cmd', required=True)
    up = sub.add_parser('up')
    up.add_argument('name')
    up.add_argument('--machine-type', required=True)
    up.add_argument('--zone', default=ZONE)
    up.add_argument('--hours', type=float, default=1.5)
    up.add_argument('--spot', action='store_true')
    up.add_argument('--disk-gb', type=int, default=50)
    up.add_argument('--image-family', default=IMAGE_FAMILY)
    up.add_argument('--image-project', default=IMAGE_PROJECT)
    ip = sub.add_parser('ip')
    ip.add_argument('name')
    ip.add_argument('--wait', type=int, default=1200)
    down = sub.add_parser('down')
    down.add_argument('name')
    zones = sub.add_parser('zones')
    zones.add_argument('machine_type')
    quota = sub.add_parser('quota')
    quota.add_argument('region')
    args = parser.parse_args()
    {'up': cmd_up, 'ip': cmd_ip, 'down': cmd_down, 'zones': cmd_zones, 'quota': cmd_quota}[args.cmd](args)


if __name__ == '__main__':
    main()
