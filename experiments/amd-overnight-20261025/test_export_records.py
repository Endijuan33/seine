#!/usr/bin/env python3
import unittest

from export_records import records


def fixture():
    result = {'hardware': {'CPU family': '26', 'Model': '2', 'CPU(s)': '64',
        'Socket(s)': '1', 'Model name': 'AMD EPYC 9355P'}, 'runs': {},
        'binary_provenance': {role: {'source_revision': role, 'binary_sha256': role}
                              for role in ('control', 'candidate')},
        'landing_status': 'Research only', 'numerical_gain_gate_passed': False}
    for warm in (False, True):
        for role in ('control', 'candidate'):
            for pair in range(1, 5):
                seconds = pair * 10
                hashes = pair * pair * 100
                result['runs'][f'{warm}-{role}-{pair}'] = {
                    'binary': role, 'warm': warm, 'pair': pair,
                    'wall_seconds': seconds, 'completed_hashes_lower_bound': hashes,
                    'completed_hps_lower_bound': hashes / seconds,
                    'starting_threads': 32, 'selected_threads': 48,
                    'kernel_cache': {'host': {'os': 'linux', 'kernel_version': 'fixture'}},
                    'first_streaming_confirmation_observed_seconds': 190,
                    'log_sha256': 'log', 'window_sha256': 'window'}
    return result


class RecordTests(unittest.TestCase):
    def test_ratio_of_totals_and_scope_preserve_lower_bound(self):
        result = records(fixture())
        self.assertEqual(len(result['records']), 4)
        for record in result['records']:
            self.assertEqual(record['completed_hashes_lower_bound'], 3000)
            self.assertEqual(record['invocation_seconds'], 100)
            self.assertEqual(record['hps'], 30)  # Arithmetic mean would be 25.
            self.assertTrue(record['hps_is_lower_bound'])
            self.assertEqual(record['selection'], 'automatic_cpu_count_and_kernel')
            self.assertEqual(record['power_scope'], 'unmeasured')
            self.assertIsNone(record['package_watts'])
            self.assertIsNone(record['hashes_per_package_joule'])
            self.assertIsNone(record['workers'])
            self.assertEqual(len(record['observations']), 4)

    def test_missing_pair_cannot_be_exported(self):
        data = fixture()
        del data['runs']['False-control-4']
        with self.assertRaises(AssertionError):
            records(data)

    def test_duplicate_pair_cannot_replace_a_missing_pair(self):
        data = fixture()
        data['runs']['False-control-4']['pair'] = 3
        with self.assertRaises(AssertionError):
            records(data)

    def test_other_hardware_cannot_inherit_this_memory_qualification(self):
        data = fixture()
        data['hardware']['Model'] = '17'
        with self.assertRaises(AssertionError):
            records(data)


if __name__ == '__main__':
    unittest.main()
