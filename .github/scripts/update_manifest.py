#!/usr/bin/env python3
"""Writes a release's update.json: for each platform the download URL, size,
SHA-256 and an Ed25519 signature the launcher checks before installing.

The signed text is exactly what crates/launcher/src/update.rs verifies:

    exile-update\\n<version>\\n<platform>\\n<sha256>\\n

Usage:
    update_manifest.py --version 0.2.0 --key signing-key.pem --base-url URL \\
        [--notes notes.txt] [--client 0.1.0] --out update.json \\
        windows-x86_64=dist/exile.exe linux-x86_64=dist/exile

Needs OpenSSL 3 on PATH (for `pkeyutl -rawin`).
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
import subprocess
import sys
import tempfile


def sign(key: str, message: bytes) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        msg = os.path.join(tmp, "statement")
        sig = os.path.join(tmp, "signature")
        with open(msg, "wb") as f:
            f.write(message)
        subprocess.run(["openssl", "pkeyutl", "-sign", "-inkey", key, "-rawin", "-in", msg, "-out", sig], check=True)
        with open(sig, "rb") as f:
            raw = f.read()
    if len(raw) != 64:
        sys.exit(f"unexpected signature length {len(raw)}: is the key Ed25519?")
    return base64.b64encode(raw).decode()


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--version", required=True)
    p.add_argument("--key", required=True, help="Ed25519 private key (PEM)")
    p.add_argument("--base-url", required=True, help="where the files are downloadable, without the file name")
    p.add_argument("--notes", help="text file with the release notes")
    p.add_argument("--client", help="bundled Exile Client version, for display")
    p.add_argument("--out", required=True)
    p.add_argument("files", nargs="+", help="platform=path")
    a = p.parse_args()

    platforms = {}
    for spec in a.files:
        platform, _, path = spec.partition("=")
        if not path:
            sys.exit(f"expected platform=path, got {spec!r}")
        with open(path, "rb") as f:
            data = f.read()
        digest = hashlib.sha256(data).hexdigest()
        statement = f"exile-update\n{a.version}\n{platform}\n{digest}\n".encode()
        platforms[platform] = {
            "url": f"{a.base_url.rstrip('/')}/{os.path.basename(path)}",
            "size": len(data),
            "sha256": digest,
            "signature": sign(a.key, statement),
        }

    notes = ""
    if a.notes and os.path.exists(a.notes):
        with open(a.notes, encoding="utf-8") as f:
            notes = f.read().strip()
    manifest = {
        "version": a.version,
        "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d"),
        "notes": notes,
        "platforms": platforms,
    }
    if a.client:
        manifest["client"] = a.client
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    print(f"wrote {a.out}: {', '.join(platforms)}")


if __name__ == "__main__":
    main()
