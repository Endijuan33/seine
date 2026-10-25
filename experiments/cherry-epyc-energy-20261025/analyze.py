#!/usr/bin/env python3
"""Recompute package-energy results from hashed evidence and export a public summary."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import bench_cpu_energy as energy


def read(path):
    return json.loads(path.read_text())


def verify(folder):
    manifest = read(folder / 'manifest.json')
    for name, digest in manifest.items():
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == digest, name
    return len(manifest)


def analyze(root):
    qualification = read(root / 'qualification/summary.json')
    result = {'scope': 'Whole-invocation CPU package energy; excludes separately unmeasured DRAM and wall losses',
        'hardware': qualification, 'binary_sha256': read(root / 'expected-files.json')['seine'],
        'source_revision': (root / 'source-revision.txt').read_text().strip(), 'phases': {}}
    parity = (root / 'results/reference-parity.log').read_text().splitlines()
    assert len([line for line in parity if line.startswith('PASS nonce=')]) == 21
    for name in ('results', 'smt-results'):
        folder = root / name
        if not folder.exists():
            continue
        files = verify(folder)
        outcome = read(folder / 'outcome.json')
        assert outcome['status'].startswith('COMPLETE') and not outcome['cleanup_failures'], outcome
        sweep = folder / 'sweep'
        manifest = read(sweep / 'sweep.json')
        assert manifest['status'] == 'complete' and len(manifest['completed']) == len(manifest['plan'])
        summary = energy.summarize_sweep(sweep, manifest['plan'], .95)
        records = []
        for leg in manifest['plan']:
            record, report = energy.load_measurement(sweep / leg['directory'])
            assert record['binary_sha256'] == result['binary_sha256']
            records.append({**leg, 'hashes': record['completed_hashes'],
                'seconds': record['invocation_secs'], 'package_joules': record['total_package_joules'],
                'raw_energy_sha256': hashlib.sha256((sweep / leg['directory'] / 'energy.json').read_bytes()).hexdigest(),
                'benchmark_sha256': record['benchmark_sha256']})
        result['phases'][name] = {'verified_files': files, 'outcome': outcome,
            'points': summary['points'], 'observed_peak_threads': summary['observed_peak_threads'],
            'efficiency_shortlist_threads': summary['efficiency_shortlist_threads'], 'observations': records,
            'paired_metrics': {key: value['metrics'] for key, value in summary['comparisons_to_baseline'].items()}}
    folder = root / 'power-results'
    if folder.exists():
        files = verify(folder)
        outcome = read(folder / 'outcome.json')
        assert outcome['status'] == 'COMPLETE' and outcome['settings_restored'] and not outcome['cleanup_failures']
        protocol = read(folder / 'protocol.json')
        for pair in range(1, 5):
            for role in ('baseline', 'candidate'):
                cap = read(folder / f'pair-{pair:02d}-{role}-cap.json')
                assert cap['before_uw'] == cap['after_uw'] == cap['requested_uw'] == protocol['caps_uw'][role]
        comparison = energy.compare([folder / f'pair-{i:02d}-baseline' for i in range(1, 5)],
                                    [folder / f'pair-{i:02d}-candidate' for i in range(1, 5)])
        metrics = comparison['metrics']
        joules = metrics['invocation_joules_per_completed_hash']['comparison']
        rate = metrics['invocation_completed_hashes_per_second']['comparison']
        accepted = (joules['sign_consistency']['candidate_lower_pairs'] == 4
                    and joules['paired_geometric_delta_ci95_pct'][1] < 0
                    and rate['paired_geometric_delta_ci95_pct'][0] >= -5)
        result['power'] = {'verified_files': files, 'protocol': protocol, 'outcome': outcome,
                           'metrics': metrics, 'predeclared_gain_gate_passed': accepted}
    return result


def plot(result, destination):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    fig, axes = plt.subplots(1, 3, figsize=(12, 4), layout='constrained')
    panels = [('completed_hps', 'Completed hashes / second'),
              ('package_watts', 'Measured CPU package watts'),
              ('joules_per_hash', 'Package joules / hash (lower is better)')]
    for name, phase in result['phases'].items():
        points = phase['points']
        for ax, (metric, title) in zip(axes, panels):
            ax.plot([p['threads'] for p in points], [p[metric] for p in points], 'o-',
                    label='Physical-core sweep' if name == 'results' else 'SMT sweep')
            ax.set(xlabel='Workers (2 GiB each)', title=title)
            ax.grid(alpha=.2)
    axes[0].legend(fontsize=8)
    fig.suptitle('EPYC 9355P · 12 × DDR5-4800 · streaming kernel · stock 280 W cap\n'
                 'Whole-invocation measurements; package energy is not wall-plug energy', fontsize=11)
    fig.savefig(destination / 'efficiency.png', dpi=180)
    fig.savefig(destination / 'efficiency.svg')
    plt.close(fig)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('collected', type=Path)
    parser.add_argument('output_dir', type=Path)
    parser.add_argument('--plot', action='store_true')
    args = parser.parse_args()
    report = analyze(args.collected)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
    if args.plot:
        plot(report, args.output_dir)
    print(json.dumps({key: value['points'] for key, value in report['phases'].items()}, indent=2))
