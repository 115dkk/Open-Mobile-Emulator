# SPDX-License-Identifier: GPL-2.0-or-later
import unittest
from pathlib import Path
from device_profile import parse_profile
from apply_device_profile import merge_properties


class DeviceProfileTests(unittest.TestCase):
    def setUp(self):
        self.text = (Path(__file__).resolve().parents[2] / 'manifests/device-profile.prop').read_text()
        self.values = parse_profile(self.text)

    def test_reject_security_or_sdk_overrides(self):
        for key in ('ro.boot.verifiedbootstate', 'ro.build.version.sdk', 'ro.adb.secure'):
            with self.assertRaises(ValueError):
                parse_profile(self.text + f'\n{key}=1\n')

    def test_reject_missing_duplicate_and_inconsistent_identity(self):
        for text in (self.text.replace('ro.product.model=SM-S948B\n', ''),
                     self.text + '\nro.product.model=other\n',
                     self.text.replace('ro.product.device=m3q', 'ro.product.device=other')):
            with self.assertRaises(ValueError):
                parse_profile(text)

    def test_migration_preserves_execution_properties_and_is_idempotent(self):
        original = 'ro.build.version.sdk=33\nro.product.cpu.abi=x86_64\nro.adb.secure=1\nro.product.model=old\nro.product.model=duplicate\nro.product.vendor.model=Generic Android-x86_64\nro.product.device=x86_64\n'
        merged = merge_properties(original, self.values, system=True)
        self.assertEqual(merged, merge_properties(merged, self.values, system=True))
        for line in original.splitlines()[:3]:
            self.assertIn(line + '\n', merged)
        self.assertEqual(merged.count('ro.product.model='), 1)
        self.assertIn('ro.product.vendor.model=Generic Android-x86_64\n', merged)
        self.assertIn('ro.product.device=x86_64\n', merged)


if __name__ == '__main__':
    unittest.main()
