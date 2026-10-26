"""Exercise receiver job transitions using a harmless socket client, no mining."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


class ReceiverTests(unittest.TestCase):
    def run_case(self, grace, rotate=True, reset=False, malformed=False, fee_switch=False):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        fake = root / 'fixture-miner'
        fake.write_text('#!' + sys.executable + '\n' + textwrap.dedent('''
            import json,os,socket,struct,sys,time
            from urllib.parse import urlsplit
            endpoint=urlsplit(sys.argv[sys.argv.index('--pool-url')+1])
            address=sys.argv[sys.argv.index('--address')+1]
            connection=socket.create_connection((endpoint.hostname,endpoint.port))
            stream=connection.makefile('r')
            def send(value):
                connection.sendall((json.dumps(value)+'\\n').encode())
            def submit(job, nonce, request):
                send({'id':request,'method':'submit','params':{'job_id':job['job_id'],'nonce':nonce,'claimed_hash':'00'*32}})
            send({'id':1,'method':'login','params':{'address':'fixture-fee' if '--fee-switch' in sys.argv else address}})
            original=None
            for line in stream:
                message=json.loads(line)
                if message.get('id')==2 and '--malformed' in sys.argv:
                    connection.sendall(b'{not-json}\\n')
                    time.sleep(30)
                if message.get('id')==2 and '--fee-switch' in sys.argv:
                    connection.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack('hh' if os.name=='nt' else 'ii',1,0))
                    stream.close()
                    connection.close()
                    connection=socket.create_connection((endpoint.hostname,endpoint.port))
                    stream=connection.makefile('r')
                    send({'id':10,'method':'login','params':{'address':address}})
                    for replacement in stream:
                        replacement=json.loads(replacement)
                        if replacement.get('method')=='job':
                            job=replacement['params']
                            submit(job,job['nonce_start'],3)
                        elif replacement.get('id')==3:
                            time.sleep(30)
                if message.get('id')==2 and '--reset' in sys.argv:
                    connection.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack('hh' if os.name=='nt' else 'ii',1,0))
                    stream.close()
                    connection.close()
                    time.sleep(30)
                if message.get('method')=='job':
                    job=message['params']
                    if original is None:
                        original=job
                        submit(job,job['nonce_start'],2)
                    else:
                        submit(original,original['nonce_start']+1,3)
                        submit(job,job['nonce_start'],4)
                        submit({'job_id':'unknown'},0,5)
                        time.sleep(30)
        '''))
        fake.chmod(0o700)
        command = [sys.executable, str(Path(__file__).with_name('bench_pool_work.py')),
                   '--output', str(root / 'result'), '--miner', 'seine', '--executable', str(fake),
                   '--seconds', '1', '--warmup', '0', '--window-origin', 'launch',
                   '--stale-grace', str(grace), '--job-secs', '0.1' if rotate else '0']
        if reset:
            command += ['--', '--reset']
        elif malformed:
            command += ['--', '--malformed']
        elif fee_switch:
            command += ['--', '--fee-switch']
        if os.name == 'nt':
            # Exercise the real native Job Object launcher with a harmless Python
            # client, inserting its script before the miner-style CLI arguments.
            command[command.index('--executable')+1] = sys.executable
            wrapper = (
                f'import sys; sys.path.insert(0, {str(Path(__file__).resolve().parent)!r}); '
                'import windows_job; original=windows_job.WindowsJobProcess; '
                f'windows_job.WindowsJobProcess=lambda argv, output: original([argv[0], {str(fake)!r}, *argv[1:]], output); '
                'import bench_pool_work; sys.argv=["bench_pool_work.py", *sys.argv[1:]]; '
                'raise SystemExit(bench_pool_work.main())'
            )
            command = [sys.executable, '-c', wrapper, *command[2:]]
        result = subprocess.run(command, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        work = json.loads((root / 'result/work.json').read_text())
        wire = [json.loads(line) for line in (root / 'result/wire.jsonl').read_text().splitlines()]
        acks = {row['message']['id']: row['message']['result'] for row in wire
                if row['direction'] == 'out' and row['message'].get('id') in [2,3,4,5]}
        return work, acks

    def test_replacement_preserves_old_header_but_rejects_stale_work(self):
        work, acks = self.run_case(0)
        self.assertEqual(work['protocol_errors'], [])
        first, stale, fresh, unknown = work['shares']
        self.assertEqual(stale['job'], first['job'])
        self.assertEqual(stale['header_base'], first['header_base'])
        self.assertNotEqual(fresh['header_base'], first['header_base'])
        self.assertNotEqual(fresh['job']['nonce_start'], first['job']['nonce_start'])
        self.assertTrue(stale['stale'])
        self.assertFalse(acks[3]['accepted'])
        self.assertTrue(acks[4]['accepted'])
        self.assertFalse(acks[5]['accepted'])
        self.assertIsNone(unknown['job'])
        self.assertEqual(work['rejected_receipts_in_window'], 2)

    def test_grace_allows_only_known_superseded_job(self):
        work, acks = self.run_case(2)
        self.assertTrue(acks[3]['accepted'])
        self.assertFalse(acks[5]['accepted'])
        self.assertEqual(work['stale_receipts_in_window'], 1)
        self.assertEqual(work['rejected_receipts_in_window'], 1)

    def test_default_static_case_never_replaces_job(self):
        work, acks = self.run_case(0, rotate=False)
        self.assertEqual(len(work['shares']), 1)
        self.assertTrue(acks[2]['accepted'])
        self.assertEqual(work['shares'][0]['job']['template_id'], 'fixture-template-user')
        self.assertEqual(work['shares'][0]['header_base'], work['header_base'])
        self.assertEqual(work['rejected_receipts_in_window'], 0)

    def test_unexpected_reset_during_measurement_is_not_hidden(self):
        work, _ = self.run_case(0, reset=True)
        self.assertEqual(work['protocol_errors'], [])
        self.assertTrue(work['connection_review_required'])
        resets = [event for event in work['connection_events'] if event['kind'] == 'peer_reset' and event['phase'] == 'measurement']
        self.assertEqual(len(resets), 1)
        self.assertEqual(resets[0]['phase'], 'measurement')
        self.assertEqual(resets[0]['role'], 'user')
        self.assertGreaterEqual(resets[0]['elapsed'], work['shares'][0]['elapsed'])
        self.assertEqual(len(work['shares']), 1, 'A disconnected miner earns no phantom work')

    def test_malformed_message_remains_a_protocol_error(self):
        work, _ = self.run_case(0, rotate=False, malformed=True)
        self.assertTrue(work['protocol_errors'])
        self.assertFalse(work['connection_review_required'])

    def test_fee_reset_and_user_reconnect_preserve_both_work_streams(self):
        work, _ = self.run_case(0, rotate=False, fee_switch=True)
        self.assertEqual(work['protocol_errors'], [])
        self.assertTrue(work['connection_review_required'])
        self.assertEqual([share['user_work'] for share in work['shares']], [False, True])
        self.assertNotEqual(work['shares'][0]['header_base'], work['shares'][1]['header_base'])
        resets = [event for event in work['connection_events'] if event['kind'] == 'peer_reset' and event['phase'] == 'measurement']
        self.assertEqual(len(resets), 1)
        self.assertEqual(resets[0]['role'], 'fee')


if __name__ == '__main__':
    unittest.main()
