"""Alert lifecycle, privacy, delivery and threshold tests; no external messages."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import alerts


class AlertsTests(unittest.TestCase):
    def setUp(self):
        self.state = {'incidents': {}}
        self.sent = []
        self.saved = []
        self.item = alerts.issue('diagnostics', 'v1', 'Three installations with errors')

    def tick(self, now, active=None, known=None, sender=None):
        return alerts.advance(self.state, [self.item] if active is None else active,
                              {'diagnostics'} if known is None else known, now,
                              sender or (lambda *args: self.sent.append(args)),
                              lambda state: self.saved.append(copy.deepcopy(state)))

    def test_confirmation_repeat_and_recovery(self):
        self.tick(1000)
        self.tick(1100)
        self.assertEqual(len(self.sent), 0)
        self.tick(1300)
        self.assertEqual(self.sent[-1][0], 'FIRING')
        self.tick(1600)
        self.assertEqual(len(self.sent), 1)
        self.tick(1300 + alerts.REPEAT_SECONDS)
        self.assertEqual(self.sent[-1][0], 'REMINDER')
        self.tick(16000, [])
        self.tick(16300, [])
        self.assertEqual(self.sent[-1][0], 'CLEARED')
        self.assertEqual(self.state['incidents'], {})

    def test_transient_does_not_notify(self):
        self.tick(1000)
        self.tick(1300, [])
        self.assertEqual(self.sent, [])
        self.assertEqual(self.state['incidents'], {})

    def test_monitor_downtime_does_not_confirm_a_pending_incident(self):
        self.tick(1000)
        self.tick(10000)
        self.assertEqual(self.sent, [])
        self.tick(10300)
        self.assertEqual(self.sent[-1][0], 'FIRING')

    def test_failed_delivery_is_not_acknowledged_and_is_retried(self):
        self.tick(1000)
        with self.assertRaises(OSError):
            self.tick(1300, sender=lambda *args: (_ for _ in ()).throw(OSError('offline')))
        self.assertIsNone(self.state['incidents'][self.item['key']]['last_sent'])
        self.tick(1600)
        self.assertEqual(self.sent[-1][0], 'FIRING')
        self.tick(1900, [])
        with self.assertRaises(OSError):
            self.tick(2200, [], sender=lambda *args: (_ for _ in ()).throw(OSError('offline')))
        self.assertIn(self.item['key'], self.state['incidents'])
        self.tick(2500, [])
        self.assertEqual(self.sent[-1][0], 'CLEARED')

    def test_unknown_query_does_not_clear_and_breaks_recovery_confirmation(self):
        self.tick(1000)
        self.tick(1300)
        self.tick(1600, [])
        self.tick(1900, [], known=set())
        self.tick(2200, [])
        self.assertEqual(len(self.sent), 1)
        self.tick(2500, [])
        self.assertEqual(self.sent[-1][0], 'CLEARED')

    def test_restart_preserves_deduplication(self):
        self.tick(1000)
        self.tick(1300)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'state.json'
            alerts.save(path, self.state)
            self.state = json.loads(path.read_text())
            self.tick(1600)
            self.assertEqual(len(self.sent), 1)

    def test_delivery_budget_eventually_sends_backlog(self):
        items = [alerts.issue('diagnostics', str(i), 'error') for i in range(15)]
        self.tick(1000, items)
        self.tick(1300, items)
        self.assertEqual(len(self.sent), 10)
        self.tick(1600, items)
        self.assertEqual(len(self.sent), 15)

    def test_aggregate_thresholds_and_unknown_categories(self):
        results = {'infrastructure': False, 'quality': {'versions': [
            {'version': 'v1', 'sustained_inventory_only_installations': 4},
            {'version': 'v2', 'sustained_inventory_only_installations': 5}]},
            'diagnostics': {'recent_error_versions': [
                {'version': 'v1', 'affected_installations': 2},
                {'version': 'v2', 'affected_installations': 3}], 'recent_unclean_versions': []},
            'performance': None}
        active, known = alerts.evaluate(results)
        self.assertEqual({i['category'] for i in active}, {'monitor', 'infrastructure', 'quality', 'diagnostics'})
        self.assertNotIn('performance', known)
        self.assertNotIn('ingestion', known)
        results['performance'] = {'minutes_since_last_report': 120, 'performance_cohorts': []}
        active, known = alerts.evaluate(results)
        self.assertIn('ingestion', {i['category'] for i in active})
        results['performance']['minutes_since_last_report'] = None
        self.assertIn('ingestion', {i['category'] for i in alerts.evaluate(results)[0]})

    def test_regression_requires_breadth_not_one_bad_rig(self):
        cohort = dict(version='v2', baseline_version='v1', hardware={'cpu_model': 'CPU'},
                      build=[], settings={'backend': 'cpu'}, installations=5,
                      affected_installations=2, median_ratio=.7)
        results = {'performance': {'minutes_since_last_report': 0, 'performance_cohorts': [cohort]}}
        self.assertEqual(alerts.evaluate(results)[0], [])
        cohort['affected_installations'] = 3
        self.assertEqual(len(alerts.evaluate(results)[0]), 1)
        cohort['median_ratio'] = .81
        self.assertEqual(alerts.evaluate(results)[0], [])

    def test_transport_disables_mentions_and_bounds_content(self):
        class Response:
            status = 200
            def read(self, size): return b'{"id":"test-message"}'
            def __enter__(self): return self
            def __exit__(self, *args): pass
        with patch('urllib.request.build_opener') as opener:
            opener.return_value.open.return_value = Response()
            alerts.deliver('https://discord.com/api/webhooks/fake/test', 'FIRING', self.item, 1000)
            request = opener.return_value.open.call_args.args[0]
            self.assertTrue(request.full_url.endswith('?wait=true'))
            body = json.loads(request.data)
            self.assertEqual(body['allowed_mentions'], {'parse': []})
            self.assertLessEqual(len(body['content']), 1900)
        self.assertNotIn('@', alerts.label('@everyone **https://private/**'))
        for url in ('http://discord.com/api/webhooks/a/b', 'https://attacker.test/api/webhooks/a/b', 'https://discord.com/other'):
            with self.assertRaises(ValueError): alerts.validate_webhook(url)
        alerts.validate_webhook('https://discord.com/api/webhooks/a/b')


if __name__ == '__main__':
    unittest.main()
