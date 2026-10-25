#!/usr/bin/env python3
"""Build an independent libargon2 reference cache after timed mining has stopped.

The same deterministic fixture lets one computed reference validate matching
nonces across miners and repeated runs. Never expose this cache to a miner.
Each process requires about 2 GiB. Run on a dedicated, otherwise idle verifier
host and reserve memory for the OS; this is not part of measured mining.
"""
import argparse
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import time

from verify_pool_work import lock_cache, reference_hasher

_hash_nonce = None


def initialize():
    global _hash_nonce
    _, _hash_nonce = reference_hasher()


def compute(item):
    key, header, nonce = item
    return key, _hash_nonce(bytes.fromhex(header), nonce)


def save(path, cache):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(cache, sort_keys=True) + '\n')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('work', type=Path, nargs='+')
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--workers', type=int, default=1)
    args = parser.parse_args()
    # Admission is intentionally Linux-only; ordinary single-process verification
    # remains usable on any host with libargon2.
    import re
    available = int(re.search(r'MemAvailable:\s+(\d+)', Path('/proc/meminfo').read_text())[1]) * 1024
    cpus = len(os.sched_getaffinity(0))
    if not 1 <= args.workers <= cpus or args.workers * 2.25 * 1024**3 > available * 0.75:
        parser.error('worker request exceeds CPU affinity or reserved-memory allowance')
    # Refuse concurrent writers; the verifier cache is evidence, not a work queue.
    cache_guard = lock_cache(args.cache)
    cache = json.loads(args.cache.read_text()) if args.cache.exists() else {}
    jobs = {}
    inputs = []
    for path in args.work:
        raw = path.read_bytes()
        work = json.loads(raw)
        inputs.append({'path': str(path), 'sha256': hashlib.sha256(raw).hexdigest()})
        for share in work['shares']:
            header = share.get('header_base', work['header_base'])
            nonce = share['params']['nonce']
            if len(bytes.fromhex(header)) != 92 or type(nonce) is not int or not 0 <= nonce < 2**64:
                raise ValueError('malformed reference input in ' + str(path))
            key = header + ':' + str(nonce)
            if key not in cache:
                jobs[key] = (key, header, nonce)
    started = time.monotonic()
    audit = {'schema': 1, 'inputs': inputs, 'workers': args.workers,
             'initial_cache_entries': len(cache), 'unique_missing': len(jobs),
             'completed': 0, 'status': 'running',
             'reference': 'libargon2 Argon2id v19, m=2097152 KiB, t=1, p=1, output=32 bytes'}
    audit_path = args.cache.with_suffix('.build.json')
    audit_path.write_text(json.dumps(audit, indent=2) + '\n')
    try:
        with multiprocessing.get_context('spawn').Pool(args.workers, initializer=initialize) as pool:
            for key, value in pool.imap_unordered(compute, jobs.values(), chunksize=1):
                cache[key] = value
                audit['completed'] += 1
                if audit['completed'] % 32 == 0:
                    save(args.cache, cache)
                    audit['elapsed_seconds'] = time.monotonic() - started
                    audit_path.write_text(json.dumps(audit, indent=2) + '\n')
                    print(f"Reference hashes: {audit['completed']}/{len(jobs)}", flush=True)
        audit['status'] = 'complete'
    except BaseException:
        audit['status'] = 'interrupted'
        raise
    finally:
        save(args.cache, cache)
        audit['elapsed_seconds'] = time.monotonic() - started
        audit_path.write_text(json.dumps(audit, indent=2) + '\n')
    print(json.dumps(audit), flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
