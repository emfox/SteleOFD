#!/usr/bin/env python3
"""
SteleOFD Android package builder.

The reader is one activity that draws with `eframe`, so the package needs very
little: a manifest naming that activity, the native library it loads, and the
signature an installable archive must carry. This tool assembles exactly that
and calls the SDK's own tools for the steps that belong to them, rather than
carrying a Gradle project for three files.

The archive is signed with a debug key generated per run. It installs on a
device that allows sideloading, which is what a release page offers; it is not
a Play Store upload, and the key is not kept.

Usage:
    python3 scripts/android_package.py --sdk DIR --lib PATH --out DIR
    python3 scripts/android_package.py selftest

Exit code: 0 on success, 1 on failure.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

PACKAGE = "org.steleofd.reader"
ACTIVITY = "android.app.NativeActivity"
MIN_SDK = 24
TARGET_SDK = 34
VERSION_CODE = 1
VERSION_NAME = "0.1.0"
ARCHIVE_NAME = "steleofd-reader-android.apk"
# The name `System.loadLibrary` looks for, which is the crate's library name.
LIBRARY = "steleofd_reader"

MANIFEST = f"""<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{PACKAGE}"
    android:versionCode="{VERSION_CODE}"
    android:versionName="{VERSION_NAME}">
    <uses-sdk android:minSdkVersion="{MIN_SDK}" android:targetSdkVersion="{TARGET_SDK}" />
    <application android:label="SteleOFD Reader" android:hasCode="false">
        <activity
            android:name="{ACTIVITY}"
            android:exported="true"
            android:configChanges="orientation|keyboardHidden|screenSize"
            android:screenOrientation="unspecified">
            <meta-data android:name="android.app.lib_name" android:value="steleofd_reader" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"""

# One ABI for now: the arm64 devices the app targets, and the only target the
# workflow builds. Adding another is another `--lib` and another directory.
ABI_DIR = "lib/arm64-v8a"


def build_tool(sdk: Path, name: str) -> str:
    """The SDK's copy of one build tool, newest build-tools first."""
    found = sorted((sdk / "build-tools").glob(f"*/{name}"))
    if not found:
        raise SystemExit(f"error: {name} is not in {sdk / 'build-tools'}")
    return str(found[-1])


def java_tool(name: str) -> str:
    """`keytool` ships with the JDK rather than the SDK, so it is found by PATH."""
    found = shutil.which(name)
    if found is None:
        raise SystemExit(f"error: {name} is not on PATH")
    return found


def run(command: list[str]) -> None:
    """Run one tool, and fail loudly: a half-built archive is not published."""
    printable = " ".join(command)
    print(f"+ {printable}")
    code = subprocess.run(command, check=False).returncode
    if code != 0:
        raise SystemExit(f"error: {printable} failed with {code}")


def write_manifest(work: Path) -> Path:
    """Write the manifest out, so the packager reads it from a file."""
    manifest = work / "AndroidManifest.xml"
    manifest.write_text(MANIFEST, encoding="utf-8")
    return manifest


def write_archive(work: Path, library: Path, compiled: Path) -> Path:
    """Add the library to the archive the packager built, and return it.

    The manifest inside must stay the binary form the packager wrote: the
    signer reads the minimum SDK out of it, and source text has no chunks.
    """
    unsigned = work / "unsigned.apk"
    shutil.copyfile(compiled, unsigned)
    with zipfile.ZipFile(unsigned, "a", zipfile.ZIP_DEFLATED) as archive:
        archive.write(library, f"{ABI_DIR}/{library.name}")
    return unsigned


def keystore(work: Path) -> Path:
    """A throwaway key: the release page is not a store, and keys are not kept."""
    path = work / "debug.keystore"
    run([
        java_tool("keytool"), "-genkeypair",
        "-keystore", str(path),
        "-alias", "steleofd",
        "-storepass", "steleofd",
        "-keypass", "steleofd",
        "-keyalg", "RSA",
        "-keysize", "2048",
        "-validity", "10000",
        "-dname", "CN=SteleOFD Reader",
    ])
    return path


def package(sdk: Path, library: Path, out: Path) -> Path:
    """Compile, align and sign the archive into `out/android`."""
    work = out / "android"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    manifest = write_manifest(work)

    # The resource table has to be the SDK's own binary form: the signer reads
    # the minimum SDK out of a compiled manifest, so the archive cannot be a
    # plain zip of the source text.
    base = build_tool(sdk, "aapt")
    assets = work / "assets"
    assets.mkdir()
    compiled = work / "compiled.apk"
    platform = sorted((sdk / "platforms").glob("android-*"))[-1]
    run([
        base, "package", "-f", "-M", str(manifest),
        "-I", str(platform / "android.jar"), "-A", str(assets), "-F", str(compiled),
    ])

    unsigned = write_archive(work, library, compiled)
    # Alignment must happen before signing: a signer rewrites the archive, and
    # an unaligned entry cannot be mapped by the loader afterwards.
    aligned = work / "aligned.apk"
    run([build_tool(sdk, "zipalign"), "-f", "4", str(unsigned), str(aligned)])

    key = keystore(work)
    signed = work / ARCHIVE_NAME
    run([
        build_tool(sdk, "apksigner"), "sign",
        "--ks", str(key),
        "--ks-pass", "pass:steleofd",
        "--ks-key-alias", "steleofd",
        "--key-pass", "pass:steleofd",
        "--out", str(signed),
        str(aligned),
    ])
    # A signature that does not verify is an archive no device will install,
    # so the build fails here rather than at the downloader.
    run([build_tool(sdk, "apksigner"), "verify", str(signed)])
    return signed


def selftest() -> int:
    """The parts that need no SDK: the manifest and the archive layout."""
    import tempfile

    assert f'package="{PACKAGE}"' in MANIFEST
    assert f'android:value="{LIBRARY}"' in MANIFEST
    assert f'android:name="{ACTIVITY}"' in MANIFEST
    assert 'android:hasCode="false"' in MANIFEST
    assert MIN_SDK < TARGET_SDK
    assert ABI_DIR == "lib/arm64-v8a"

    # The archive is laid out the same way here, so a change to the layout is
    # caught without an SDK or a toolchain.
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        library = work / f"lib{LIBRARY}.so"
        library.write_bytes(b"\x7fELF")
        compiled = work / "compiled.apk"
        with zipfile.ZipFile(compiled, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.write(write_manifest(work), "AndroidManifest.xml")
        unsigned = write_archive(work, library, compiled)
        with zipfile.ZipFile(unsigned) as archive:
            names = set(archive.namelist())
    expected = {
        "AndroidManifest.xml",
        f"{ABI_DIR}/lib{LIBRARY}.so",
    }
    assert names == expected, names
    print("selftest passed")
    return 0


def main(argv: list[str]) -> int:
    if argv and argv[0] == "selftest":
        return selftest()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True, help="Android SDK root")
    parser.add_argument("--lib", type=Path, required=True, help="the built .so")
    parser.add_argument("--out", type=Path, required=True, help="where the archive goes")
    args = parser.parse_args(argv)

    if not args.lib.is_file():
        raise SystemExit(f"error: {args.lib} is not a file")
    signed = package(args.sdk, args.lib, args.out)
    print(f"built {signed}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
