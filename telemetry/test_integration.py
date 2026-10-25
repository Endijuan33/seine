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
