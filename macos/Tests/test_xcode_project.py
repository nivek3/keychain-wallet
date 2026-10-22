"""Check signing safeguards without an account, certificate, build, or Keychain access."""
import pathlib
import plistlib
import unittest
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]


class XcodeSigningTests(unittest.TestCase):
    def test_private_group_matches_the_bundle_metadata(self):
        with (ROOT / "KeychainWallet.entitlements").open("rb") as source:
            entitlements = plistlib.load(source)
        with (ROOT / "Info.plist").open("rb") as source:
            info = plistlib.load(source)
        app_id = "$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)"
        self.assertEqual(entitlements, {
            "com.apple.application-identifier": app_id,
            "com.apple.developer.team-identifier": "$(DEVELOPMENT_TEAM)",
            "keychain-access-groups": [app_id],
        })
        self.assertEqual(info["WalletAccessGroup"], app_id)

    def test_release_run_opens_setup_without_a_debugger(self):
        scheme = ET.parse(ROOT / "KeychainWallet.xcodeproj/xcshareddata/xcschemes/KeychainWallet.xcscheme").getroot()
        launch = scheme.find("LaunchAction")
        self.assertEqual(launch.get("buildConfiguration"), "Release")
        self.assertEqual(launch.get("selectedDebuggerIdentifier"), "")
        arguments = launch.findall("CommandLineArguments/CommandLineArgument")
        self.assertEqual([(arg.get("argument"), arg.get("isEnabled")) for arg in arguments],
                         [("check-signing", "NO")])
        settings = dict(line.split(" = ", 1) for line in (ROOT / "Wallet.xcconfig").read_text().splitlines()
                        if " = " in line and not line.startswith("//"))
        self.assertEqual(settings["ENABLE_HARDENED_RUNTIME"], "YES")
        self.assertEqual(settings["CODE_SIGN_INJECT_BASE_ENTITLEMENTS"], "NO")
        self.assertEqual(settings["ENABLE_DEBUG_DYLIB"], "NO")


if __name__ == "__main__":
    unittest.main()
