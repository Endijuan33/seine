#!/usr/bin/env python3
"""Exercise a disposable local collector + PostgreSQL. Writes fixture rows."""
import json
import os
from pathlib import Path
import subprocess
import urllib.error
import urllib.request
import uuid

URL = os.environ.get('TELEMETRY_TEST_URL', 'http://127.0.0.1:9080')
DATABASE = os.environ['TELEMETRY_TEST_DATABASE']
FIXTURE = json.loads(Path(__file__).with_name('fixture.json').read_text())


def report():
    value = json.loads(json.dumps(FIXTURE))
    value['installation_id'] = uuid.uuid4().hex
    value['report_id'] = uuid.uuid4().hex
    return value


def send(value):
    body = json.dumps(value).encode()
    request = urllib.request.Request(URL + '/v1/reports', body, {'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code


def query(sql):
    return subprocess.check_output(['psql', DATABASE, '-At', '-v', 'ON_ERROR_STOP=1', '-c', sql], text=True).strip()


valid = report()
assert send(valid) == 204
assert send(valid) == 429
invalid = report()
invalid['hardware']['hostname'] = 'private-field-fixture'
assert send(invalid) == 422
assert send({}) == 422
oversized = report()
oversized['hardware']['cpu_model'] = 'x' * 40000
assert send(oversized) == 413
bad_window = report()
bad_window['window_secs'] = 0
assert send(bad_window) == 422
row = query(f"SELECT payload->>'report_id' FROM telemetry_reports WHERE report_id='{valid['report_id']}'")
assert row == valid['report_id']
assert query("SELECT median_hps FROM telemetry_backend_performance WHERE cpu_model='Example CPU (fixture)'") == '10'
assert query("SELECT count(*) FROM telemetry_reports WHERE payload::text LIKE '%private-field-fixture%'") == '0'
print('PASS: valid upload, rate limit, strict fields, empty/oversized/invalid-window rejection, database persistence and population-weighted performance view')

# The collector accepts both generations while rejecting v2 fields disguised as v1.
legacy = report()
legacy['schema_version'] = 1
legacy['hardware'].pop('resources')
legacy['hardware']['cpu_model'] = 'Legacy CPU (fixture)'
for backend in legacy['backends']:
    backend['settings'].pop('cpu_backing_mib')
    backend['settings'].pop('cpu_affinity')
assert send(legacy) == 204
bad_version = report()
bad_version['schema_version'] = 1
assert send(bad_version) == 422


def insert(value, hours):
    payload = json.dumps(value).replace("'", "''")
    query("INSERT INTO telemetry_reports (report_id,installation_id,received_at,payload) "
          f"VALUES ('{value['report_id']}','{value['installation_id']}',"
          f"now()-interval '{hours} hours','{payload}'::jsonb)")


def analysis(filename):
    return json.loads(subprocess.check_output(
        ['psql', DATABASE, '-qAt', '-v', 'ON_ERROR_STOP=1', '-f',
         str(Path(__file__).with_name(filename))], text=True))


# Exact settings are not enough: environments, builds, RAM and resource limits
# must separate otherwise identical CPUs. Free RAM remains a diagnostic snapshot.
for cohort in range(5):
    for installation in range(5):
        identity = uuid.uuid4().hex
        for index, hours in enumerate((26, 13, 1)):
            sample = report()
            sample['installation_id'] = identity
            sample['hardware']['cpu_model'] = 'Analysis CPU (fixture)'
            if cohort == 1:
                sample['hardware']['runtime_environment'] = 'wsl'
            elif cohort == 2:
                sample['build_features'] = 'cpu-only'
            elif cohort == 3:
                sample['hardware']['memory_gib'] = 128
            elif cohort == 4:
                sample['hardware']['resources']['effective_memory_gib'] = 32
            sample['hardware']['resources']['available_memory_gib'] = 20 + index
            sample['backends'][0]['hashes'] = 3600 * (10 + index * 10 + cohort * 100)
            # One report has two identical devices; it still counts as one report.
            if index == 0:
                duplicate = json.loads(json.dumps(sample['backends'][0]))
                duplicate['instance'] = 2
                sample['backends'].append(duplicate)
            insert(sample, hours)

candidates = analysis('agent_report.sql')['performance_candidates']
candidates = [c for c in candidates if c['cpu'] == 'Analysis CPU (fixture)']
assert len(candidates) == 5, candidates
assert sorted(c['median_hps'] for c in candidates) == [20, 120, 220, 320, 420]
assert all(c['installations'] == 5 for c in candidates)
view = json.loads(query("SELECT json_agg(v) FROM telemetry_backend_performance v "
                        "WHERE cpu_model='Analysis CPU (fixture)'"))
assert sorted(v['median_hps'] for v in view) == [20, 120, 220, 320, 420]
assert all(v['installations'] == 5 for v in view)

# Recent inventory-only reporters cannot be confused with healthy sparse cohorts.
identities = [uuid.uuid4().hex for _ in range(5)]
for index, identity in enumerate(identities):
    for hours in (4, 2, 1):
        empty = report()
        empty['installation_id'] = identity
        empty['miner_version'] = 'quality-fixture'
        empty['backends'] = []
        insert(empty, hours)
    quality = analysis('data_quality.sql')
    assert quality['status'] == ('error' if index == 4 else 'ok'), quality
for identity in identities:
    recovered = report()
    recovered['installation_id'] = identity
    recovered['miner_version'] = 'quality-fixture'
    insert(recovered, 0)
assert analysis('data_quality.sql')['status'] == 'ok'
print('PASS: schema 1/2 compatibility, environment/build/RAM/limit cohort separation, '
      'per-report deduplication, stable cohort thresholds, sustained empty-sample detection and recovery')
