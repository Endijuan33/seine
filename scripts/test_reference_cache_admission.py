"""Keep macOS memory admission conservative across4KiB and16KiB hosts."""
import unittest
from build_reference_cache import darwin_available_memory


class AdmissionTests(unittest.TestCase):
    def test_counts_reclaimable_pages_without_compressed_or_swap_memory(self):
        for size in [4096, 16384]:
            with self.subTest(page_size=size):
                output = f'''Mach Virtual Memory Statistics: (page size of {size} bytes)
Pages free: 100.
Pages active: 900.
Pages inactive: 200.
Pages speculative: 50.
Pages purgeable: 90.
Pages occupied by compressor: 800.
Swapins: 999999.
'''
                self.assertEqual(darwin_available_memory(output), 350 * size)

    def test_missing_or_unrecognized_stats_fail_closed(self):
        with self.assertRaises((TypeError, AttributeError)):
            darwin_available_memory('unknown memory-statistics output')


if __name__ == '__main__':
    unittest.main()
