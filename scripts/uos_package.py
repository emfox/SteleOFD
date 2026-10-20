#!/usr/bin/env python3
"""
SteleOFD UOS package builder.

UOS Desktop V20 is Debian 10 underneath, so the package is a `.deb`: the
control file, the binary, a desktop entry that puts it in the launcher, and an
icon. This tool writes that archive from its parts, so the release carries no
`debian/` directory and no `dpkg-buildpackage` step for four files.

The dependency list is the binary's own: `font-kit` links fontconfig, which
links freetype, and both arrive with any desktop install. Nothing else is
asked for, because a dependency that is already on every target system only
makes an install fail when a name is wrong.

The binary must come from a glibc 2.28 build. A package built on a newer
system installs and then refuses to start, so `--check-glibc` reads the
symbols the binary asks for and fails the build rather than the user.

Usage:
    python3 scripts/uos_package.py --binary PATH --version 0.2.0 --out DIR
    python3 scripts/uos_package.py --binary PATH --version 0.2.0 --out DIR --check-glibc
    python3 scripts/uos_package.py selftest

Exit code: 0 on success, 1 on failure.
"""

from __future__ import annotations

import argparse
import gzip
import io
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

PACKAGE = "steleofd-reader"
VERSION_FALLBACK = "0.2.0"
ARCHITECTURE = "amd64"
MAINTAINER = "SteleOFD <steleofd@example.invalid>"
SECTION = "office"
PRIORITY = "optional"
HOMEPAGE = "https://github.com/emfox/SteleOFD"

# The glibc UOS Desktop V20 ships, which is also Debian 10's. A binary needing
# anything above this installs cleanly and then fails to start.
GLIBC_BASELINE = (2, 28)

# The reader links fontconfig through font-kit; freetype arrives with it. Both
# are on every desktop install, so naming them costs nothing and catches the
# system where one was stripped.
DEPENDS = "libc6 (>= 2.28), libfontconfig1, libfreetype6"

# Where Debian puts an application's parts. The binary goes in `bin`, the
# launcher entry where the menu reads it, and the icon in the size the theme
# asks for first.
BIN_DIR = "usr/bin"
DESKTOP_DIR = "usr/share/applications"
ICON_DIR = "usr/share/icons/hicolor/scalable/apps"
DOC_DIR = f"usr/share/doc/{PACKAGE}"

EXECUTABLE = "steleofd-reader"
ARCHIVE_NAME = f"{PACKAGE}-uos.deb"

DESKTOP_ENTRY = f"""[Desktop Entry]
Type=Application
Name=SteleOFD Reader
GenericName=OFD Reader
Comment=Read and export OFD documents
Exec={EXECUTABLE} %f
Icon={PACKAGE}
Terminal=false
Categories={SECTION.capitalize()};Viewer;Graphics;
MimeType=application/ofd;
StartupNotify=true
"""

# A document with a folded corner, in the flat two-tone the desktop's own icon
# theme uses. Drawn rather than shipped as a bitmap so it stays sharp at the
# size a launcher asks for.
ICON_SVG = f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64">
  <path d="M12 4h28l12 12v44a4 4 0 0 1-4 4H16a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4z"
        fill="#f2f2f2" stroke="#3d3d3d" stroke-width="3"/>
  <path d="M40 4l12 12H44a4 4 0 0 1-4-4z" fill="#c8c8c8" stroke="#3d3d3d"
        stroke-width="3" stroke-linejoin="round"/>
  <g fill="#3d3d3d">
    <rect x="20" y="26" width="24" height="3" rx="1.5"/>
    <rect x="20" y="34" width="24" height="3" rx="1.5"/>
    <rect x="20" y="42" width="16" height="3" rx="1.5"/>
  </g>
</svg>
"""


def glibc_required(binary: Path) -> set[tuple[int, int]]:
    """The `GLIBC_x.y` versions one ELF binary's dynamic symbols ask for.

    Read from the symbol table rather than by running it: a binary that cannot
    start is exactly the case this has to diagnose, and running it would only
    report that it could not start.
    """
    readelf = shutil.which("readelf") or shutil.which("objdump")
    if readelf is None:
        raise SystemExit("error: readelf is not on PATH; install binutils")

    if readelf.endswith("readelf"):
        output = subprocess.run(
            [readelf, "--version-info", "--wide", str(binary)],
            capture_output=True, text=True, check=False,
        ).stdout
    else:
        output = subprocess.run(
            [readelf, "-T", str(binary)],
            capture_output=True, text=True, check=False,
        ).stdout

    found = set()
    for major, minor in re.findall(r"GLIBC_(\d+)\.(\d+)", output):
        found.add((int(major), int(minor)))
    return found


def check_glibc(binary: Path, baseline: tuple[int, int] = GLIBC_BASELINE) -> None:
    """Fail when the binary needs a glibc newer than the target system has."""
    needed = glibc_required(binary)
    if not needed:
        # A static binary asks for nothing, which no target can be too old for.
        return
    highest = max(needed)
    if highest > baseline:
        wanted = ".".join(str(part) for part in baseline)
        got = ".".join(str(part) for part in highest)
        raise SystemExit(
            f"error: {binary} needs GLIBC_{got}, but UOS Desktop V20 ships "
            f"GLIBC_{wanted}. Build on a glibc {wanted} system."
        )


def control_file(version: str, installed_kib: int) -> str:
    """The package's own description, in the format `dpkg` reads."""
    return (
        f"Package: {PACKAGE}\n"
        f"Version: {version}\n"
        f"Architecture: {ARCHITECTURE}\n"
        f"Maintainer: {MAINTAINER}\n"
        f"Installed-Size: {installed_kib}\n"
        f"Depends: {DEPENDS}\n"
        f"Section: {SECTION}\n"
        f"Priority: {PRIORITY}\n"
        f"Homepage: {HOMEPAGE}\n"
        f"Description: OFD document reader\n"
        f" Reads GB/T 33190 OFD documents and exports them to PDF and images.\n"
        f" The rendering engine is shared with the command line tool and the\n"
        f" browser build, so a page looks the same everywhere it is opened.\n"
    )


def stage(root: Path, binary: Path, version: str) -> Path:
    """Lay the package's files out under `root` and return that root."""
    def place(relative: str, data: bytes, mode: int = 0o644) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        path.chmod(mode)
        return path

    place(
        f"{BIN_DIR}/{EXECUTABLE}",
        binary.read_bytes(),
        0o755,
    )
    place(f"{DESKTOP_DIR}/{PACKAGE}.desktop", DESKTOP_ENTRY.encode("utf-8"))
    place(f"{ICON_DIR}/{PACKAGE}.svg", ICON_SVG.encode("utf-8"))

    # Debian policy asks for these two, and a lintian run names them when they
    # are missing. The licence is the repository's own, unmodified.
    license_path = Path(__file__).resolve().parent.parent / "LICENSE"
    if license_path.is_file():
        text = license_path.read_bytes()
        place(f"{DOC_DIR}/copyright", text)
    place(
        f"{DOC_DIR}/changelog.Debian.gz",
        gzip.compress(
            f"{PACKAGE} ({version}) unstable; urgency=medium\n\n"
            f"  * Release {version}.\n\n"
            f" -- {MAINTAINER}  Thu, 01 Jan 1970 00:00:00 +0000\n".encode("utf-8"),
            mtime=0,
        ),
    )

    # `dpkg` reads the control file from the archive's own directory, not from
    # a list of files, so it sits outside the payload and is written here.
    place("DEBIAN/control", control_file(version, installed_kib(root)).encode("utf-8"))
    return root


def installed_kib(root: Path) -> int:
    """The size `dpkg` reports, which is the payload rounded up to kilobytes."""
    total = sum(
        path.stat().st_size
        for path in root.rglob("*")
        if path.is_file() and "DEBIAN" not in path.relative_to(root).parts
    )
    return (total + 1023) // 1024


def write_archive(root: Path, out: Path) -> Path:
    """Write `root` as a `.deb`, which is an `ar` archive of three members.

    The members carry a fixed timestamp and ownership, so two builds of the
    same bytes produce the same archive and a release asset can be diffed.
    """
    mode = format(0o100644, "o")

    def member(name: str, payload: bytes) -> bytes:
        # A fixed 60-byte header, then the payload, then the padding byte a
        # member of odd length needs. `dpkg` reads these offsets literally, so
        # each field is padded to its own width rather than joined by spaces.
        header = (
            f"{name:<16}"
            f"{0:<12}"
            f"{0:<6}"
            f"{0:<6}"
            f"{mode:<8}"
            f"{len(payload):<10}"
            "`\n"
        ).encode()
        assert len(header) == 60, len(header)
        return header + payload + (b"\n" if len(payload) % 2 else b"")

    # `control.tar` holds the metadata at its own root, so its entries are
    # named `./control` rather than `DEBIAN/control`; `dpkg` reads the first
    # path component as the file and rejects a directory.
    control = member("control.tar.gz", tar_tree(root / "DEBIAN", "."))
    payload = member("data.tar.gz", tar_tree(root, ".", skip=("DEBIAN",)))
    text = b"!<arch>\n" + member("debian-binary", b"2.0\n") + control + payload

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(text)
    return out


def tar_tree(root: Path, arcname: str, skip: tuple[str, ...] = ()) -> bytes:
    """One member's gzipped tar, with the metadata pinned so the bytes are stable.

    `arcname` is the directory inside the archive the files go under. `dpkg`
    unpacks `data.tar` relative to `/`, so its entries carry a leading `./`;
    `control.tar` keeps its files under `DEBIAN/` instead. `skip` names the
    top-level directories that belong to the other member.
    """
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as archive:
        for path in sorted(root.rglob("*")):
            relative = path.relative_to(root)
            if relative.parts[0] in skip:
                continue
            name = f"./{relative}" if arcname == "." else f"{arcname}/{relative}"
            info = archive.gettarinfo(str(path), arcname=name)
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = 0
            # A directory entry has to be in the archive: `dpkg` creates the
            # files an archive lists and nothing else, so a member under a
            # path with no entry of its own fails to unpack.
            info.mode = 0o755 if path.is_dir() or path.stat().st_mode & stat.S_IXUSR else 0o644
            if path.is_file():
                with path.open("rb") as handle:
                    archive.addfile(info, handle)
            else:
                archive.addfile(info)
    return gzip.compress(raw.getvalue(), mtime=0)


def package(binary: Path, version: str, out: Path, verify_glibc: bool) -> Path:
    """Stage, check and write the archive into `out`."""
    if not binary.is_file():
        raise SystemExit(f"error: {binary} is not a file")
    if verify_glibc:
        check_glibc(binary)

    staging = Path(tempfile.mkdtemp(prefix="steleofd-uos-"))
    try:
        stage(staging, binary, version)
        archive = write_archive(staging, out / ARCHIVE_NAME)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    return archive


def selftest() -> int:
    """The parts that need no toolchain: the layout and the control file."""
    assert ARCHITECTURE == "amd64"
    assert f"Exec={EXECUTABLE} %f" in DESKTOP_ENTRY
    assert f"Icon={PACKAGE}" in DESKTOP_ENTRY
    assert "MimeType=application/ofd;" in DESKTOP_ENTRY
    assert GLIBC_BASELINE == (2, 28)

    control = control_file("1.2.3", 4096)
    assert "Package: steleofd-reader" in control
    assert "Architecture: amd64" in control
    assert "Version: 1.2.3" in control
    assert "libc6 (>= 2.28)" in control
    # Every line of a control paragraph is a field or a continuation; a bare
    # line would make `dpkg` reject the package outright.
    for line in control.splitlines():
        assert line.startswith(" ") or ":" in line, line

    svg = ICON_SVG.strip()
    assert svg.startswith("<?xml")
    assert 'viewBox="0 0 64 64"' in svg

    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        binary = work / EXECUTABLE
        binary.write_bytes(b"\x7fELF")
        root = work / "stage"
        root.mkdir()
        stage(root, binary, "1.2.3")
        archive = write_archive(root, work / ARCHIVE_NAME)

        members = ar_members(archive.read_bytes())
        assert list(members) == [
            "debian-binary", "control.tar.gz", "data.tar.gz",
        ], list(members)
        assert members["debian-binary"] == b"2.0\n"

        payload = tar_names(gzip.decompress(members["data.tar.gz"]))
        assert f"./{BIN_DIR}/{EXECUTABLE}" in payload, payload
        assert f"./{DESKTOP_DIR}/{PACKAGE}.desktop" in payload, payload
        assert f"./{ICON_DIR}/{PACKAGE}.svg" in payload, payload
        assert not any(name.startswith("./DEBIAN") for name in payload), payload

        meta = tar_names(gzip.decompress(members["control.tar.gz"]))
        assert meta == ["./control"], meta

    print("selftest passed")
    return 0


def ar_members(data: bytes) -> dict[str, bytes]:
    """Split the `ar` archive a `.deb` is into its named members."""
    assert data.startswith(b"!<arch>\n"), data[:8]
    offset = 8
    members: dict[str, bytes] = {}
    while offset + 60 <= len(data):
        header = data[offset:offset + 60]
        offset += 60
        name = header[:16].decode().strip().rstrip("/")
        size_field = header[48:58].decode().strip()
        if not size_field.isdigit():
            break
        size = int(size_field)
        members[name] = data[offset:offset + size]
        # Members are padded to an even length; the pad byte is not payload.
        offset += size + (size % 2)
    return members


def tar_names(raw: bytes) -> list[str]:
    """The names one member's tar carries, in order."""
    with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
        return [member.name for member in archive.getmembers()]


def main(argv: list[str]) -> int:
    if argv and argv[0] == "selftest":
        return selftest()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="the built executable")
    parser.add_argument("--version", default=VERSION_FALLBACK, help="the package version")
    parser.add_argument("--out", type=Path, required=True, help="where the archive goes")
    parser.add_argument(
        "--check-glibc", action="store_true",
        help="fail when the binary needs a glibc newer than the target ships",
    )
    args = parser.parse_args(argv)

    archive = package(args.binary, args.version, args.out, args.check_glibc)
    print(f"built {archive}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
