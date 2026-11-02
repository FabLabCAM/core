#!/usr/bin/env python3
"""Collect notices and corresponding MPL source for the six desktop packages."""
import json
import pathlib
import shutil
import subprocess
import sys
import tarfile

out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
subprocess.run(["cargo", "about", "generate", "--locked", "packaging/licenses.hbs", "-o", str(out / "THIRD-PARTY-LICENSES.html")], check=True)
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"]))
for package in metadata["packages"]:
    root = pathlib.Path(package["manifest_path"]).parent
    if package["name"] == "epaint":
        # Include font-specific copyrights in addition to the SPDX license text.
        fonts = out / "font-licenses"
        fonts.mkdir(exist_ok=True)
        for source in (root / "fonts").glob("*.txt"):
            shutil.copy2(source, fonts / source.name)
    if "MPL" in (package["license"] or ""):
        sources = out / "third-party-sources"
        sources.mkdir(exist_ok=True)
        name = f'{package["name"]}-{package["version"]}'
        with tarfile.open(sources / f"{name}.tar.gz", "w:gz") as archive:
            for source in sorted(root.rglob("*")):
                if source.is_file() and source.name not in {".cargo-ok", ".cargo_vcs_info.json"}:
                    archive.add(source, arcname=str(pathlib.Path(name) / source.relative_to(root)))
for filename in ("LICENSE", "NOTICE"):
    shutil.copy2(filename, out / filename)
