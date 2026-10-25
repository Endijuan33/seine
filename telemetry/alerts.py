#!/usr/bin/env python3
"""Bounded, stateful Discord alerts from aggregate telemetry checks (stdlib only)."""
import argparse
from datetime import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parent
REPEAT_SECONDS = 4 * 3600
CONFIRM_SECONDS = 240
MAX_SENDS = 10


def identity(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()[:24]


def label(value):
    # Miner-controlled labels cannot inject mentions, markdown, or links.
    return ''.join(c if c.isalnum() or c in ' ._()-' else '_' for c in str(value))[:100]


def issue(category, key, summary):
    return {'category': category, 'key': category + ':' + identity(key), 'summary': summary}


def evaluate(results):
    """Unknown queries never resolve their previous incidents."""
    active, known = [], {'monitor'}
    for category, result in results.items():
        if result is None:
            active.append(issue('monitor', category, f'{category} check could not complete. Inspect the telemetry alert service journal.'))
        else:
            known.add(category)
    if results.get('infrastructure') is False:
        active.append(issue('infrastructure', 'health', 'Collector HTTPS, database, retention, disk or backup check failed. Run telemetry/deploy/check.sh --infrastructure-only.'))
    quality = results.get('quality')
    if quality is not None:
        for row in quality['versions']:
            count = row['sustained_inventory_only_installations']
            if count >= 5:
                active.append(issue('quality', row['version'], f'{label(row["version"])}: {count} installations repeatedly report inventory without hashrate samples. Check adoption and the sampling path.'))
    diagnostics = results.get('diagnostics')
    if diagnostics is not None:
        for field, title in [('recent_error_versions', 'miner errors/quarantines'), ('recent_unclean_versions', 'unclean previous runs (not proof of crashes)')]:
            for row in diagnostics[field]:
                count = row['affected_installations']
                if count >= 3:
                    active.append(issue('diagnostics', [field, row['version']], f'{label(row["version"])}: {count} installations with {title} in 24 hours. Run telemetry/agent_report.py for issue clusters.'))
    performance = results.get('performance')
    if performance is not None:
        known.add('ingestion')
        minutes = performance['minutes_since_last_report']
        if minutes is None or minutes >= 120:
            active.append(issue('ingestion', 'stale', 'No periodic release reports in the last two hours. Check ingest delivery and miner activity.'))
        for row in performance['performance_cohorts']:
            if row['median_ratio'] <= .8 and row['affected_installations'] >= max(3, .6 * row['installations']):
                key = {k: row[k] for k in ('version', 'baseline_version', 'hardware', 'build', 'settings')}
                active.append(issue('performance', key, f'{label(row["version"])} / {label(row["hardware"].get("cpu_model", "unknown CPU"))} / {label(row["settings"].get("backend"))}: paired median hashrate fell {100 * (1-row["median_ratio"]):.0f}% across {row["installations"]} installations versus {label(row["baseline_version"])}. Observational signal; verify with controlled benchmarks.'))
    return active, known


def run(command, data=None):
    return subprocess.run(command, input=data, text=True, capture_output=True, timeout=45)


def collect():
    results = {}
    try:
        health = run(['bash', str(ROOT / 'deploy/check.sh'), '--infrastructure-only'])
        results['infrastructure'] = health.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        results['infrastructure'] = None
    compose = ['docker', 'compose', '--project-directory', str(ROOT / 'deploy'),
               '--env-file', str(ROOT / 'deploy/.env'), '-f', str(ROOT / 'deploy/compose.yaml'),
               'exec', '-T', 'db', 'psql', '-X', '-U', 'telemetry', '-d', 'telemetry',
               '-qAt', '-v', 'ON_ERROR_STOP=1']
    for category, name in [('quality', 'data_quality.sql'), ('diagnostics', 'diagnostics.sql'), ('performance', 'alerts.sql')]:
        try:
            result = run(compose, (ROOT / name).read_text())
            results[category] = json.loads(result.stdout) if result.returncode == 0 else None
        except (OSError, subprocess.TimeoutExpired, ValueError):
            results[category] = None
    return results


def save(path, state):
    temporary = path.with_suffix('.tmp')
    with temporary.open('w') as stream:
        json.dump(state, stream)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def advance(state, active, known, now, send, persist):
    """Two spaced observations to open/clear, retry failed delivery next check."""
    current = {item['key']: item for item in active}
    incidents = state.setdefault('incidents', {})
    if now - state.get('last_check', now) > 900:
        for record in incidents.values():
            record.pop('clear_since', None)
            if record['last_sent'] is None:
                record['first_seen'] = now
    state['last_check'] = now
    for key, item in current.items():
        record = incidents.setdefault(key, {**item, 'first_seen': now, 'last_sent': None})
        record['summary'] = item['summary']
        record.pop('clear_since', None)
        record['last_seen'] = now
    sends = 0
    # Oldest unsent notifications first; cap messages during widespread incidents.
    for key, record in sorted(list(incidents.items()), key=lambda pair: pair[1].get('last_sent') or 0):
        if record['category'] not in known:
            # A failed query also breaks consecutive confirmation of recovery.
            record.pop('clear_since', None)
            if record['last_sent'] is None:
                record['first_seen'] = now
            continue
        present = key in current
        if present:
            if now - record['first_seen'] < CONFIRM_SECONDS:
                continue
            if record['last_sent'] is not None and now - record['last_sent'] < REPEAT_SECONDS:
                continue
            transition = 'FIRING' if record['last_sent'] is None else 'REMINDER'
        else:
            if record['last_sent'] is None:
                del incidents[key]
                continue
            clear_since = record.setdefault('clear_since', now)
            if now - clear_since < CONFIRM_SECONDS:
                continue
            transition = 'CLEARED'
        if sends >= MAX_SENDS:
            break
        send(transition, record, now)
        sends += 1
        if present:
            record['last_sent'] = now
        else:
            del incidents[key]
        persist(state)  # Only acknowledge delivery after HTTP success.
    persist(state)
    return sends


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def validate_webhook(url):
    parsed = urlsplit(url)
    if (parsed.scheme != 'https' or parsed.hostname not in ('discord.com', 'discordapp.com')
            or not parsed.path.startswith('/api/webhooks/') or parsed.username or parsed.password
            or parsed.port not in (None, 443) or parsed.fragment):
        raise ValueError('Expected an HTTPS Discord webhook URL')


def deliver(url, transition, record, now):
    validate_webhook(url)
    parsed = urlsplit(url)
    query = [(key, value) for key, value in parse_qsl(parsed.query) if key != 'wait']
    url = urlunsplit(parsed._replace(query=urlencode(query + [('wait', 'true')])))
    stamp = datetime.fromtimestamp(now, ZoneInfo('America/New_York')).strftime('%Y-%m-%d %H:%M %Z')
    suffix = ('\nThreshold cleared or observations aged out; this does not prove every miner recovered.'
              if transition == 'CLEARED' else '')
    content = f'Seine telemetry — {transition} — {record["category"]}\n{record["summary"]}{suffix}\n{stamp} · incident {record["key"]}'
    body = json.dumps({'content': content[:1900], 'allowed_mentions': {'parse': []}}).encode()
    request = urllib.request.Request(url, body, {'Content-Type': 'application/json', 'User-Agent': 'seine-telemetry-alerts/1'})
    with urllib.request.build_opener(NoRedirect).open(request, timeout=10) as response:
        if response.status != 200:
            raise RuntimeError('Notification delivery failed')
        result = json.loads(response.read(65536))
        if not result.get('id'):
            raise RuntimeError('Notification confirmation missing')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dry-run', action='store_true', help='Read checks and print candidates; no state writes or messages')
    parser.add_argument('--state-dir', type=Path, default=Path('/var/lib/seine-telemetry-alerts'))
    args = parser.parse_args()
    os.umask(0o077)
    if args.dry_run:
        active, known = evaluate(collect())
        print(json.dumps({'active': active, 'observed_categories': sorted(known)}, indent=2))
        return 0
    url = os.environ.get('DISCORD_WEBHOOK_URL', '').strip()
    validate_webhook(url)
    args.state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    with (args.state_dir / 'lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print('Another alert check is running; skipping.')
            return 0
        path = args.state_dir / 'state.json'
        state = json.loads(path.read_text()) if path.exists() else {'incidents': {}}
        active, known = evaluate(collect())
        sends = advance(state, active, known, time.time(),
                        lambda *values: deliver(url, *values), lambda value: save(path, value))
        print(f'Telemetry alerts: {len(active)} active thresholds, {sends} delivered, {len(known)} observed categories.')
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        # urllib exceptions may contain the webhook token; never log their text.
        print(f'Telemetry alert check failed ({type(error).__name__}); delivery remains pending. Check configuration and service connectivity.', file=sys.stderr)
        sys.exit(1)
