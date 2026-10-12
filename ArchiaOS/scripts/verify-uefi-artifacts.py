#!/usr/bin/env python3
"""Strict structural audit for the production x86-64 UEFI loader and kernel."""
import hashlib
import os
import struct
import sys

U64_MAX = (1 << 64) - 1
PT_LOAD = 1
PF_X, PF_W, PF_R = 1, 2, 4
KERNEL_HIGH_HALF = 0xFFFFFFFF80000000


def fail(message):
    raise SystemExit("binary audit failed: " + message)


def read_file(path):
    try:
        with open(path, "rb") as stream:
            data = stream.read()
    except OSError as exc:
        fail(f"cannot read {path}: {exc}")
    if not data:
        fail(f"{path} is empty")
    return data


def check_pe_uefi(path):
    data = read_file(path)
    if len(data) < 64 or data[:2] != b"MZ":
        fail(f"{path}: missing DOS MZ header")
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if pe_offset > len(data) - 24 or data[pe_offset:pe_offset + 4] != b"PE\0\0":
        fail(f"{path}: invalid PE signature/offset")
    machine, sections, _, _, _, optional_size, _ = struct.unpack_from(
        "<HHIIIHH", data, pe_offset + 4
    )
    optional = pe_offset + 24
    if sections == 0 or optional_size < 70 or optional + optional_size > len(data):
        fail(f"{path}: malformed COFF/optional header")
    magic = struct.unpack_from("<H", data, optional)[0]
    if machine != 0x8664:
        fail(f"{path}: PE machine is 0x{machine:04x}, expected AMD64 (0x8664)")
    if magic != 0x20B:
        fail(f"{path}: optional header is not PE32+")
    subsystem = struct.unpack_from("<H", data, optional + 68)[0]
    if subsystem != 10:
        fail(f"{path}: PE subsystem is {subsystem}, expected EFI application (10)")
    section_table = optional + optional_size
    if section_table + sections * 40 > len(data):
        fail(f"{path}: section table extends beyond the file")
    for i in range(sections):
        off = section_table + i * 40
        raw_size, raw_offset = struct.unpack_from("<II", data, off + 16)
        if raw_size and (raw_offset > len(data) or raw_size > len(data) - raw_offset):
            fail(f"{path}: section {i} raw data extends beyond the file")
    print(f"PASS PE/COFF: AMD64 EFI application, {sections} sections, {len(data)} bytes")


def check_elf_kernel(path):
    data = read_file(path)
    if len(data) < 64 or data[:4] != b"\x7fELF":
        fail(f"{path}: missing ELF signature/header")
    if data[4] != 2 or data[5] != 1 or data[6] != 1:
        fail(f"{path}: expected ELF64, little-endian, version 1")
    e_type, machine, version = struct.unpack_from("<HHI", data, 16)
    entry, phoff = struct.unpack_from("<QQ", data, 24)
    ehsize, phentsize, phnum = struct.unpack_from("<HHH", data, 52)
    if e_type not in (2, 3):
        fail(f"{path}: unsupported ELF type {e_type}; expected EXEC or DYN")
    if machine != 62 or version != 1:
        fail(f"{path}: expected x86-64 ELF (machine 62, version 1)")
    if ehsize < 64 or phentsize < 56 or phnum == 0:
        fail(f"{path}: invalid ELF/program-header sizes")
    if phoff > len(data) or phnum > (len(data) - phoff) // phentsize:
        fail(f"{path}: program-header table extends beyond the file")

    loads = []
    for i in range(phnum):
        off = phoff + i * phentsize
        p_type, flags = struct.unpack_from("<II", data, off)
        p_offset, vaddr, paddr, filesz, memsz, align = struct.unpack_from(
            "<QQQQQQ", data, off + 8
        )
        if p_type != PT_LOAD:
            continue
        if filesz > memsz:
            fail(f"{path}: PT_LOAD #{i} filesz exceeds memsz")
        if p_offset > len(data) or filesz > len(data) - p_offset:
            fail(f"{path}: PT_LOAD #{i} file range exceeds image")
        if vaddr > U64_MAX - memsz or paddr > U64_MAX - memsz:
            fail(f"{path}: PT_LOAD #{i} address range overflows u64")
        if memsz == 0:
            continue
        if vaddr < KERNEL_HIGH_HALF:
            fail(f"{path}: PT_LOAD #{i} is not in the expected high-half virtual range")
        if align not in (0, 1):
            if align & (align - 1):
                fail(f"{path}: PT_LOAD #{i} alignment is not a power of two")
            if (vaddr - p_offset) % align:
                fail(f"{path}: PT_LOAD #{i} virtual address/offset are incongruent")
        if flags & (PF_W | PF_X) == (PF_W | PF_X):
            fail(f"{path}: PT_LOAD #{i} is writable and executable (W^X violation)")
        loads.append((i, vaddr, paddr, memsz, flags))
    if not loads:
        fail(f"{path}: no non-empty PT_LOAD segments")

    for n, a in enumerate(loads):
        for b in loads[n + 1:]:
            _, av, ap, am, _ = a
            _, bv, bp, bm, _ = b
            if av < bv + bm and bv < av + am:
                fail(f"{path}: virtual PT_LOAD ranges overlap")
            if ap < bp + bm and bp < ap + am:
                fail(f"{path}: physical PT_LOAD ranges overlap")

    if not any(flags & PF_X and vaddr <= entry < vaddr + memsz
               for _, vaddr, _, memsz, flags in loads):
        fail(f"{path}: entry point is not inside an executable PT_LOAD segment")
    print(f"PASS ELF64: x86-64 high-half kernel, entry=0x{entry:016x}, "
          f"{len(loads)} load segments, {len(data)} bytes")


def main():
    if len(sys.argv) != 3:
        fail("usage: verify-uefi-artifacts.py <BOOTX64.EFI> <kernel.elf>")
    for path in sys.argv[1:]:
        if not os.path.isfile(path):
            fail(f"missing file: {path}")
    check_pe_uefi(sys.argv[1])
    check_elf_kernel(sys.argv[2])
    for path in sys.argv[1:]:
        digest = hashlib.sha256(read_file(path)).hexdigest()
        print(f"SHA256 {path}: {digest}")


if __name__ == "__main__":
    main()
