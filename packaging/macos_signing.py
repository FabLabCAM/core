"""Stable Developer ID signing when configured; explicit ad-hoc fallback for CI PRs.

Never imports certificates or changes keychain ACLs. Use a pre-provisioned identity.
"""
import os
import re
import subprocess


def configuration(env):
    identity = env.get("FABLABCAM_CODESIGN_IDENTITY", "").strip()
    team = env.get("FABLABCAM_CODESIGN_TEAM", "").strip()
    required = env.get("FABLABCAM_REQUIRE_DEVELOPER_ID", "0") == "1"
    if not identity:
        if required or team:
            raise ValueError("Developer ID identity required; refusing ad-hoc fallback")
        return None
    if not re.fullmatch(r"[A-Fa-f0-9]{40}", identity) or not re.fullmatch(r"[A-Z0-9]{10}", team):
        raise ValueError("Use the certificate SHA-1 fingerprint and ten-character Apple Team ID")
    return identity, team


def sign(app, config, run=subprocess.run):
    if config is None:
        run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
    else:
        identity, team = config
        # Sign inside out. Keep identifiers stable across versions and architectures.
        targets = [(app / "Contents" / "MacOS" / binary, identifier) for binary, identifier in (
            ("fablabcam", "org.fablabcam.cli"),
            ("fablabcam-view", "org.fablabcam.viewer"),
            ("fablabcam-studio-ui", "org.fablabcam.studio"),
        )] + [(app, "org.fablabcam.studio")]
        for target, identifier in targets:
            run(["codesign", "--force", "--sign", identity, "--identifier", identifier,
                 "--options", "runtime", "--timestamp", str(target)], check=True)
            info = run(["codesign", "-d", "--verbose=4", str(target)], check=True,
                       capture_output=True, text=True).stderr
            lines = info.splitlines()
            if (f"TeamIdentifier={team}" not in lines
                    or f"Identifier={identifier}" not in lines
                    or not any(line.startswith("Authority=Developer ID Application:") for line in lines)):
                raise RuntimeError("Signed code is not the expected Developer ID Application identity")
    run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
