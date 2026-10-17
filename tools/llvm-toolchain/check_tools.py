"""Reject host dynamic libraries in the actual selected LLVM executables."""

import pathlib
import struct
import subprocess
import sys


def check_static_elf(path):
    data = path.read_bytes()
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01":
        raise ValueError(f"{path}: expected a little-endian ELF64 executable")
    header = struct.unpack_from("<HHIQQQIHHHHHH", data, 16)
    if header[1] != 62:
        raise ValueError(f"{path}: expected an x86_64 executable")
    offset, entry_size, entry_count = header[4], header[8], header[9]
    if entry_size != 56 or entry_count == 0:
        raise ValueError(f"{path}: invalid ELF program headers")
    if offset + entry_size * entry_count > len(data):
        raise ValueError(f"{path}: truncated ELF program headers")
    for index in range(entry_count):
        segment = struct.unpack_from("<IIQQQQQQ", data, offset + index * entry_size)
        if segment[0] == 3:  # PT_INTERP
            raise ValueError(f"{path}: PT_INTERP requires a host dynamic loader")
        if segment[0] != 2:  # PT_DYNAMIC
            continue
        start, size = segment[2], segment[5]
        if size % 16 or start + size > len(data):
            raise ValueError(f"{path}: invalid ELF dynamic segment")
        for dynamic_offset in range(start, start + size, 16):
            tag, _ = struct.unpack_from("<qQ", data, dynamic_offset)
            if tag == 0:  # DT_NULL
                break
            if tag == 1:  # DT_NEEDED
                raise ValueError(f"{path}: DT_NEEDED requires a host shared library")


def main():
    for argument in sys.argv[1:]:
        name, value = argument.split("=", 1)
        path = pathlib.Path(value)
        if name != "compiler_driver":
            check_static_elf(path)
        if name != "llvm":
            subprocess.run(
                [str(path), "--version"],
                check=True,
                stdout=subprocess.DEVNULL,
                timeout=30,
            )


if __name__ == "__main__":
    main()
