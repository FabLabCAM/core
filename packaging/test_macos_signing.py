import pathlib
import subprocess
import unittest
from macos_signing import configuration, sign


class SigningTests(unittest.TestCase):
    def test_release_never_silently_falls_back(self):
        self.assertIsNone(configuration({}))
        for env in ({"FABLABCAM_REQUIRE_DEVELOPER_ID": "1"},
                    {"FABLABCAM_CODESIGN_IDENTITY": "-"},
                    {"FABLABCAM_CODESIGN_TEAM": "ABCDEFGHIJ"}):
            with self.assertRaises(ValueError):
                configuration(env)

    def test_stable_identifiers_and_developer_id_verification(self):
        calls = []
        def run(cmd, **kwargs):
            calls.append(cmd)
            if "-d" in cmd:
                identifier = calls[-2][calls[-2].index("--identifier") + 1]
                return subprocess.CompletedProcess(cmd, 0, stderr=f"TeamIdentifier=ABCDEFGHIJ\nIdentifier={identifier}\nAuthority=Developer ID Application: Test\n")
        sign(pathlib.Path("Test.app"), ("a" * 40, "ABCDEFGHIJ"), run)
        signed = [c for c in calls if "--sign" in c]
        self.assertEqual(len(signed), 4)
        self.assertTrue(all("--timestamp" in c and "runtime" in c and "--deep" not in c for c in signed))
        self.assertEqual(signed[-1][signed[-1].index("--identifier") + 1], "org.fablabcam.studio")

    def test_wrong_certificate_or_team_is_rejected(self):
        def run(cmd, **kwargs):
            return subprocess.CompletedProcess(cmd, 0, stderr="TeamIdentifier=WRONG\nAuthority=Apple Development: Test\n")
        with self.assertRaises(RuntimeError):
            sign(pathlib.Path("Test.app"), ("a" * 40, "ABCDEFGHIJ"), run)


if __name__ == "__main__":
    unittest.main()
