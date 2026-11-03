#!/usr/bin/env python3
"""Build desktop archives and native installers from already compiled binaries.

The CI builds natively for each target. No credentials or user configuration are
packaged. The same notices accompany the application, CLI and backplot viewer.
"""
import argparse
import os
import pathlib
import plistlib
import shutil
import subprocess
import tarfile
import tomllib

parser = argparse.ArgumentParser()
parser.add_argument("--target", required=True)
parser.add_argument("--platform", choices=["macos", "windows", "linux"], required=True)
parser.add_argument("--arch", choices=["x64", "arm64"], required=True)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1]
os.chdir(root)
version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
name = f"FabLabCAM-{version}-{args.platform}-{args.arch}"
out = root / "dist" / "release"
work = root / "target" / "package" / name
if work.exists():
    shutil.rmtree(work)
work.mkdir(parents=True)
out.mkdir(parents=True, exist_ok=True)
bins = root / "target" / args.target / "release"
suffix = ".exe" if args.platform == "windows" else ""
for binary in ("fablabcam", "fablabcam-view", "fablabcam-studio-ui"):
    shutil.copy2(bins / (binary + suffix), work / (binary + suffix))
shutil.copytree(root / "target" / "notices", work, dirs_exist_ok=True)
shutil.copytree(root / "profiles", work / "profiles")
(work / "README.txt").write_text(
    f"FabLabCAM {version} — {args.platform} {args.arch}\n\n"
    "Studio: fablabcam-studio-ui\nCLI: fablabcam help\nViewer: fablabcam-view\n"
    "Documentation: https://fablabcam.github.io/docs/\n"
    "Source: https://github.com/FabLabCAM/core\n\n"
    "Apache-2.0; dependencies retain their licenses. See THIRD-PARTY-LICENSES.html.\n"
    "Packages are not commercially signed or notarized.\n", encoding="utf-8")

def run(*command):
    subprocess.run([str(c) for c in command], check=True)

if args.platform == "macos":
    app = work / "FabLabCAM Studio.app"
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    resources = contents / "Resources"
    resources.mkdir()
    for binary in ("fablabcam", "fablabcam-view", "fablabcam-studio-ui"):
        shutil.move(work / binary, contents / "MacOS" / binary)
    shutil.copytree(root / "target" / "notices", resources, dirs_exist_ok=True)
    shutil.copytree(root / "profiles", resources / "profiles")
    icon = root / "target" / "package" / "icon.png"
    run("python3", "docs/make-icon.py", icon)
    iconset = root / "target" / "package" / "AppIcon.iconset"
    iconset.mkdir(exist_ok=True)
    for size in (16, 32, 64, 128, 256, 512):
        for multiplier in (1, 2):
            filename = f"icon_{size}x{size}{'@2x' if multiplier == 2 else ''}.png"
            run("sips", "-z", size * multiplier, size * multiplier, icon, "--out", iconset / filename)
    run("iconutil", "-c", "icns", iconset, "-o", resources / "AppIcon.icns")
    info = dict(CFBundleIdentifier="org.fablabcam.studio", CFBundleName="FabLabCAM Studio",
                CFBundleDisplayName="FabLabCAM Studio", CFBundleExecutable="fablabcam-studio-ui",
                CFBundleIconFile="AppIcon", CFBundlePackageType="APPL",
                CFBundleShortVersionString=version, CFBundleVersion=version,
                LSMinimumSystemVersion="11.0", NSHighResolutionCapable=True,
                LSApplicationCategoryType="public.app-category.graphics-design")
    info["CFBundleDocumentTypes"] = [dict(CFBundleTypeName="FabLabCAM drawing", CFBundleTypeExtensions=["cb", "dxf", "svg"], CFBundleTypeRole="Editor", LSHandlerRank="Alternate")]
    (contents / "Info.plist").write_bytes(plistlib.dumps(info))
    run("codesign", "--force", "--deep", "--sign", "-", app)
    run("codesign", "--verify", "--deep", "--strict", app)
    run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", work, out / f"{name}.zip")
    os.symlink("/Applications", work / "Applications")
    run("hdiutil", "create", "-volname", f"FabLabCAM {version}", "-srcfolder", work, "-ov", "-format", "UDZO", out / f"{name}.dmg")
    run("hdiutil", "verify", out / f"{name}.dmg")
elif args.platform == "windows":
    shutil.make_archive(str(out / name), "zip", work.parent, work.name)
    nsis = shutil.which("makensis") or str(pathlib.Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "NSIS" / "makensis.exe")
    run(nsis, "/INPUTCHARSET", "UTF8", f"/DVERSION={version}", f"/DARCH={args.arch}", f"/DSOURCE={work}", f"/DOUTPUT={out / (name + '-Setup.exe')}", root / "packaging/windows.nsi")
else:
    with tarfile.open(out / f"{name}.tar.gz", "w:gz") as archive:
        archive.add(work, arcname=name)
    deb = root / "target" / "package" / (name + "-deb")
    if deb.exists():
        shutil.rmtree(deb)
    (deb / "DEBIAN").mkdir(parents=True)
    bindir = deb / "usr" / "bin"
    bindir.mkdir(parents=True)
    for binary in ("fablabcam", "fablabcam-view", "fablabcam-studio-ui"):
        shutil.copy2(work / binary, bindir / binary)
    doc = deb / "usr" / "share" / "doc" / "fablabcam"
    shutil.copytree(root / "target" / "notices", doc)
    shutil.copy2(work / "README.txt", doc / "README.txt")
    shutil.copytree(root / "profiles", doc / "profiles")
    desktop = deb / "usr" / "share" / "applications"
    desktop.mkdir(parents=True)
    (desktop / "org.fablabcam.studio.desktop").write_text("[Desktop Entry]\nType=Application\nName=FabLabCAM Studio\nComment=CAO/FAO 2.5D\nExec=fablabcam-studio-ui %F\nIcon=org.fablabcam.studio\nTerminal=false\nCategories=Graphics;Engineering;\n")
    icons = deb / "usr" / "share" / "icons" / "hicolor" / "1024x1024" / "apps"
    icons.mkdir(parents=True)
    run("python3", "docs/make-icon.py", icons / "org.fablabcam.studio.png")
    architecture = "amd64" if args.arch == "x64" else "arm64"
    (deb / "DEBIAN" / "control").write_text(
        f"Package: fablabcam\nVersion: {version}\nArchitecture: {architecture}\n"
        "Maintainer: FabLabCAM (rhinosys) <5645423+rhinosys@users.noreply.github.com>\nSection: graphics\nPriority: optional\n"
        "Depends: libc6 (>= 2.35), libgcc-s1, libstdc++6, libx11-6, libxcursor1, libxi6, libxrandr2, libxkbcommon0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1, zenity\n"
        "Recommends: fonts-dejavu-core\nHomepage: https://fablabcam.github.io/docs/\n"
        "Description: FabLabCAM Studio, CAD/CAM and G-code tools\n 2D drawing, 2.5D machining, toolpaths and stock simulation.\n")
    run("dpkg-deb", "--build", "--root-owner-group", deb, out / f"{name}.deb")
    run("dpkg-deb", "--info", out / f"{name}.deb")
print(f"Packages ready: {out}")
