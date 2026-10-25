"""Exercise receiver job transitions using a harmless socket client, no mining."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


@unittest.skipIf(os.name == 'nt', 'POSIX fixture executable; native Windows process lifecycle tested separately')
class ReceiverTests(unittest.TestCase):
    def run_case(self, grace, rotate=True, reset=False):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        fake = root / 'fixture-miner'
        fake.write_text('#!' + sys.executable + '\n' + textwrap.dedent('''
            import json,socket,struct,sys,time
            from urllib.parse import urlsplit
            endpoint=urlsplit(sys.argv[sys.argv.index('--pool-url')+1])
            address=sys.argv[sys.argv.index('--address')+1]
            connection=socket.create_connection((endpoint.hostname,endpoint.port))
            stream=connection.makefile('r')
            def send(value):
                connection.sendall((json.dumps(value)+'\\n').encode())
            def submit(job, nonce, request):
                send({'id':request,'method':'submit','params':{'job_id':job['job_id'],'nonce':nonce,'claimed_hash':'00'*32}})
            send({'id':1,'method':'login','params':{'address':address}})
            original=None
            for line in stream:
                message=json.loads(line)
                if message.get('id')==2 and '--reset' in sys.argv:
                    connection.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack('ii',1,0))
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
        self.assertTrue(work['protocol_errors'])


if __name__ == '__main__':
    unittest.main()
