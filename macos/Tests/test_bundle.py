"""Public synthetic profile metadata only. Does not sign code or touch Keychain."""
import datetime
import pathlib
import plistlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parents[2] / "scripts/prepare-bundle.py"


class BundleTests(unittest.TestCase):
    def prepare(self, authorized_group="PREFIX.com.example.Wallet", expired=False):
        folder = tempfile.TemporaryDirectory(prefix="wallet-public-profile-test-")
        self.addCleanup(folder.cleanup)
        root = pathlib.Path(folder.name)
        app = root / "KeychainWallet.app"
        (app / "Contents").mkdir(parents=True)
        profile = {
            "ApplicationIdentifierPrefix": ["PREFIX"], "TeamIdentifier": ["DIFFERENT_TEAM"],
            "ExpirationDate": datetime.datetime(2000 if expired else 2099, 1, 1),
            "Entitlements": {"com.apple.application-identifier": "PREFIX.com.example.*",
                             "keychain-access-groups": [authorized_group]},
        }
        source = root / "public-profile.plist"
        with source.open("wb") as file:
            plistlib.dump(profile, file)
        result = subprocess.run([sys.executable, str(SCRIPT), str(source), "com.example.Wallet", str(app)],
                                capture_output=True, check=False)
        return result, root, app

    def test_uses_app_identifier_prefix_and_only_private_group(self):
        result, root, app = self.prepare()
        self.assertEqual(result.returncode, 0, result.stderr)
        with (root / "entitlements.plist").open("rb") as source:
            entitlements = plistlib.load(source)
        self.assertEqual(entitlements, {
            "com.apple.application-identifier": "PREFIX.com.example.Wallet",
            "com.apple.developer.team-identifier": "DIFFERENT_TEAM",
            "keychain-access-groups": ["PREFIX.com.example.Wallet"],
        })
        with (app / "Contents/Info.plist").open("rb") as source:
            self.assertEqual(plistlib.load(source)["WalletAccessGroup"], "PREFIX.com.example.Wallet")

    def test_rejects_unauthorized_or_expired_profiles(self):
        for group, expired in [("PREFIX.other.*", False), ("PREFIX.com.example.*", True)]:
            with self.subTest(group=group, expired=expired):
                result, _, app = self.prepare(group, expired)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((app / "Contents/Info.plist").exists())


if __name__ == "__main__":
    unittest.main()
