#!/usr/bin/env python3
"""Azure VM leases for cohort and GPU bring-up runs (same interface as aws.py / do.py / gcp.py).

up:    create a resource group with one VM. cloud-init installs a task-only SSH key for root
       and schedules a guest power-off at the deadline; a DevTest auto-shutdown deallocates
       the VM at the same time and a local systemd timer deletes the whole resource group
       independently (a stopped-but-not-deallocated VM still bills).
ip:    print the public IPv4.
down:  delete the resource group (VM, disk, NIC, IP, NSG) and the local key.
sizes: list the AMD GPU sizes a region offers and any restriction on them.
quota: show the GPU family usage/limit in a region (requests go through the portal's
       "My quotas" page; new subscriptions start at 0 for every GPU family).

Needs `az login` first. AMD RDNA sizes (Linux pay-as-you-go, cheapest region):
  Standard_NV24ads_V710_v5   1x Radeon PRO V710 24 GB (RDNA3, gfx1101)  eastus   ~$2.00/h
  Standard_NV12ads_V710_v5   1/2 V710 (12 GB)                            eastus   ~$1.00/h
  Standard_NG32ads_V620_v1   1x Radeon PRO V620 32 GB (RDNA2, gfx1030)  eastus2  ~$2.54/h
Both families need Secure Boot off for the AMD kernel module, so VMs are created with
--security-type Standard. Driver install: scripts/cohort/remote_rocm_install.sh
(GFX=gfx1101 for V710, MODE=azure-v620 for V620). Receipts: ~/.cache/seine-hashrate/leases/<name>.json.
"""
import argparse
import datetime as dt
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
AZ = os.environ.get('AZ') or shutil.which('az') or '/usr/bin/az'


def az(*args, check=True, timeout=900):
    result = subprocess.run([AZ, *args, '-o', 'json'], capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        if check:
            raise SystemExit(f"az {' '.join(args[:3])}: {result.stderr.strip()[:800]}")
        return None
    return json.loads(result.stdout) if result.stdout.strip() else None


def receipt_path(name):
    return LEASES / f'{name}.json'


def save(receipt):
    receipt_path(receipt['name']).write_text(json.dumps(receipt, indent=2))


def cloud_init(public_key, minutes):
    # Azure writes the admin key into root's authorized_keys with a forced command that only
    # prints "Please login as the user seine". The lease key IS that admin key, so a plain
    # append is skipped by the duplicate check and root keeps the forced-command line only
    # (seen on the first V710 lease, 2026-10-28). Drop the forced-command line, then append,
    # and allow root key logins as gcp.py does.
    return f"""#cloud-config
runcmd:
  - mkdir -p /root/.ssh && chmod 700 /root/.ssh
  - sed -i '/command=/d' /root/.ssh/authorized_keys 2>/dev/null || true
  - grep -qF '{public_key}' /root/.ssh/authorized_keys 2>/dev/null || echo '{public_key}' >> /root/.ssh/authorized_keys
  - chmod 600 /root/.ssh/authorized_keys
  - mkdir -p /etc/ssh/sshd_config.d
  - printf 'PermitRootLogin prohibit-password\\n' > /etc/ssh/sshd_config.d/00-seine-lease.conf
  - systemctl restart ssh || systemctl restart sshd
  - shutdown -P +{minutes} 'seine lease deadline'
"""


def cmd_up(args):
    LEASES.mkdir(parents=True, exist_ok=True)
    if receipt_path(args.name).exists():
        raise SystemExit(f'lease {args.name} already exists')
    key = CACHE / f'{args.name}-ssh'
    subprocess.run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', args.name, '-f', str(key)], check=True)
    group = f'seine-{args.name}'
    receipt = {'name': args.name, 'provider': 'azure', 'region': args.region, 'key': str(key),
               'instance_type': args.size, 'image': args.image, 'hours': args.hours, 'market': 'on-demand',
               'resource_group': group}
    save(receipt)
    unit = f'seine-lease-{args.name}'
    subprocess.run(['systemd-run', '--user', f'--on-active={int(args.hours * 3600)}', f'--unit={unit}',
                    sys.executable, str(Path(__file__).resolve()), 'down', args.name],
                   check=True, capture_output=True)
    try:
        az('group', 'create', '-n', group, '-l', args.region, '--tags', 'seine-cohort=1')
        receipt['group_created'] = True
        save(receipt)
        minutes = max(5, int(args.hours * 60) - 5)
        public_key = key.with_suffix('.pub').read_text().strip()
        with tempfile.NamedTemporaryFile('w', suffix='.yaml', delete=False) as user_data:
            user_data.write(cloud_init(public_key, minutes))
            user_data_path = user_data.name
        try:
            vm = az('vm', 'create', '-g', group, '-n', args.name, '--size', args.size, '--image', args.image,
                    '--admin-username', 'seine', '--ssh-key-values', str(key.with_suffix('.pub')),
                    '--public-ip-sku', 'Standard', '--nsg-rule', 'SSH', '--os-disk-size-gb', str(args.os_disk_gb),
                    '--storage-sku', 'Premium_LRS', '--security-type', 'Standard',
                    '--custom-data', user_data_path, '--tags', 'seine-cohort=1')
        finally:
            Path(user_data_path).unlink(missing_ok=True)
        receipt['instance_id'] = vm.get('id')
        receipt['ip'] = vm.get('publicIpAddress')
        save(receipt)
        at = (dt.datetime.now(dt.timezone.utc) + dt.timedelta(hours=args.hours)).strftime('%H%M')
        if az('vm', 'auto-shutdown', '-g', group, '-n', args.name, '--time', at, check=False) is None:
            print('note: auto-shutdown schedule refused; relying on guest shutdown + local timer', file=sys.stderr)
    except SystemExit:
        cmd_down(args)
        raise
    print(json.dumps({'name': args.name, 'resource_group': group, 'ip': receipt.get('ip'),
                      'delete_after_hours': args.hours}))


def cmd_ip(args):
    receipt = json.loads(receipt_path(args.name).read_text())
    deadline = time.time() + args.wait
    while True:
        vm = az('vm', 'show', '-d', '-g', receipt['resource_group'], '-n', args.name, check=False)
        ip = (vm or {}).get('publicIps')
        state = (vm or {}).get('powerState')
        if ip and state == 'VM running':
            print(ip)
            return
        if time.time() > deadline:
            raise SystemExit(f'not ready: {state}')
        time.sleep(10)


def cmd_down(args):
    path = receipt_path(args.name)
    if not path.exists():
        print(f'no lease {args.name}')
        return
    receipt = json.loads(path.read_text())
    group = receipt['resource_group']
    if receipt.get('group_created'):
        subprocess.run([AZ, 'group', 'delete', '-n', group, '--yes', '--no-wait'], capture_output=True, text=True)
        for _ in range(60):
            exists = subprocess.run([AZ, 'group', 'exists', '-n', group], capture_output=True, text=True)
            if exists.stdout.strip() == 'false':
                break
            time.sleep(10)
        else:
            raise SystemExit(f'resource group {group} not deleted')
    for suffix in ('', '.pub'):
        Path(receipt['key'] + suffix).unlink(missing_ok=True)
    subprocess.run(['systemctl', '--user', 'stop', f"seine-lease-{args.name}.timer"], capture_output=True)
    path.rename(path.with_suffix('.deleted.json'))
    print(f"deleted {args.name} ({group})")


def cmd_sizes(args):
    skus = az('vm', 'list-skus', '-l', args.region, '--resource-type', 'virtualMachines',
              '--query', "[?contains(name,'V710') || contains(name,'V620')]")
    for sku in skus or []:
        caps = {c['name']: c['value'] for c in sku.get('capabilities', [])}
        restr = ';'.join(r.get('reasonCode', '') for r in sku.get('restrictions', []))
        print(f"{sku['name']:30} vcpu={caps.get('vCPUs'):>3} mem={caps.get('MemoryGB'):>4} GiB "
              f"gpus={caps.get('GPUs', '?')} {'RESTRICTED:' + restr if restr else 'ok'}")


def cmd_quota(args):
    usage = az('vm', 'list-usage', '-l', args.region,
               '--query', "[?contains(name.value,'NVadsV710') || contains(name.value,'NGads') || name.value=='cores']")
    for row in usage or []:
        print(f"{row['name']['localizedValue']:45} {row['currentValue']:>4} / {row['limit']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='cmd', required=True)
    up = sub.add_parser('up')
    up.add_argument('name')
    up.add_argument('--size', default='Standard_NV24ads_V710_v5')
    up.add_argument('--region', default='eastus')
    up.add_argument('--image', default='Canonical:ubuntu-24_04-lts:server:latest')
    up.add_argument('--os-disk-gb', type=int, default=128)
    up.add_argument('--hours', type=float, default=2.0)
    ip = sub.add_parser('ip')
    ip.add_argument('name')
    ip.add_argument('--wait', type=int, default=900)
    down = sub.add_parser('down')
    down.add_argument('name')
    sizes = sub.add_parser('sizes')
    sizes.add_argument('--region', default='eastus')
    quota = sub.add_parser('quota')
    quota.add_argument('--region', default='eastus')
    args = parser.parse_args()
    {'up': cmd_up, 'ip': cmd_ip, 'down': cmd_down, 'sizes': cmd_sizes, 'quota': cmd_quota}[args.cmd](args)


if __name__ == '__main__':
    main()
