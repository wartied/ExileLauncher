"""Packs exile.exe into the installer: appends it to exile-setup.exe with
the trailer crates/installer/src/payload.rs reads.

    python3 pack_installer.py <exile-setup.exe> <exile.exe> <out.exe>
"""

import hashlib
import struct
import sys


def main() -> None:
    installer, launcher, out = sys.argv[1:4]
    stub = open(installer, "rb").read()
    payload = open(launcher, "rb").read()
    if not stub.startswith(b"MZ") or not payload.startswith(b"MZ"):
        sys.exit("both inputs must be Windows executables")
    trailer = hashlib.sha256(payload).digest() + struct.pack("<Q", len(payload)) + b"EXILEPK1"
    with open(out, "wb") as f:
        f.write(stub + payload + trailer)
    print(f"{out}: {len(stub) + len(payload) + len(trailer)} bytes")


if __name__ == "__main__":
    main()
