#!/usr/bin/env python3
"""Build an isolated macOS diagnostic app; never re-sign an installed release.

The opt-in get-task-allow entitlement is necessary for native core capture on
recent macOS. Distributable packages deliberately retain their normal signature.
"""
import os
import pathlib
import plistlib
import shutil
import subprocess
import sys
import tomllib

if sys.platform != "darwin":
    raise SystemExit("This diagnostic bundle requires macOS")
root = pathlib.Path(__file__).resolve().parents[1]
os.chdir(root)
version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
environment = os.environ.copy()
environment["FABLABCAM_BUILD_ID"] = subprocess.check_output(["git", "describe", "--always", "--dirty"], text=True).strip()
environment["CARGO_PROFILE_DIAGNOSTIC_SPLIT_DEBUGINFO"] = "packed"
subprocess.run(["cargo", "build", "--locked", "--profile", "diagnostic", "-p", "fablabcam-view", "--bin", "fablabcam-studio-ui"], env=environment, check=True)
output = root / "target" / "diagnostic-app"
app = output / "FabLabCAM Diagnostic.app"
if app.exists():
    shutil.rmtree(app)
binaries = app / "Contents" / "MacOS"
binaries.mkdir(parents=True)
source = root / "target" / "diagnostic" / "fablabcam-studio-ui"
shutil.copy2(source, binaries / source.name)
info = dict(CFBundleIdentifier="org.fablabcam.studio.diagnostic", CFBundleName="FabLabCAM Diagnostic",
            CFBundleExecutable=source.name, CFBundlePackageType="APPL", CFBundleShortVersionString=version,
            CFBundleVersion=version, NSHighResolutionCapable=True, LSMinimumSystemVersion="11.0")
(app / "Contents" / "Info.plist").write_bytes(plistlib.dumps(info))
entitlements = output / "diagnostic.entitlements"
entitlements.write_bytes(plistlib.dumps({"com.apple.security.get-task-allow": True}))
subprocess.run(["codesign", "--force", "--sign", "-", "--entitlements", str(entitlements), str(app)], check=True)
subprocess.run(["codesign", "--verify", "--strict", str(app)], check=True)
symbols = output / "fablabcam-studio-ui.dSYM"
if symbols.exists():
    shutil.rmtree(symbols)
shutil.copytree(source.with_suffix(".dSYM"), symbols)
for path in (binaries / source.name, symbols):
    subprocess.run(["dwarfdump", "--uuid", str(path)], check=True)
print(f"Diagnostic app: {app}\nMatching symbols: {symbols}\nCore mode remains off until explicitly armed in Preferences.")
