#!/usr/bin/env python3
"""Static audit of the x86-64 ISR stubs and their lookup table.

This catches drift between vector numbers, CPU-pushed error-code exceptions,
and the table consumed by interrupts::install without requiring a privileged
CPU or executing the kernel.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "kernel/src/interrupts.rs"
ERROR_CODE_VECTORS = {8, 10, 11, 12, 13, 14, 17, 21, 29, 30}


def fail(message: str) -> None:
    print(f"ISR audit failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    stubs = list(re.finditer(
        r'^\s*"isr_stub_(\d+):",(?P<body>.*?)(?=^\s*"\.global isr_stub_|^\s*"\.global isr_common")',
        source,
        re.MULTILINE | re.DOTALL,
    ))
    by_vector = {int(match.group(1)): match.group("body") for match in stubs}
    expected_vectors = set(range(256))
    if set(by_vector) != expected_vectors:
        missing = sorted(expected_vectors - set(by_vector))
        extra = sorted(set(by_vector) - expected_vectors)
        fail(f"stub coverage mismatch; missing={missing}, extra={extra}")

    for vector, body in by_vector.items():
        pushes = [int(value) for value in re.findall(r'"pushq \$(\d+)",', body)]
        expected = [vector] if vector in ERROR_CODE_VECTORS else [0, vector]
        if pushes != expected:
            fail(f"vector {vector}: expected push sequence {expected}, got {pushes}")

        if '"jmp isr_common",' not in body:
            fail(f"vector {vector}: does not branch to isr_common")

    table_match = re.search(
        r'"isr_stub_table:",(?P<table>.*?)options\(att_syntax\)',
        source,
        re.DOTALL,
    )
    if not table_match:
        fail("isr_stub_table not found")
    table = [int(value) for value in re.findall(r'isr_stub_(\d+)', table_match.group("table"))]
    if table != list(range(256)):
        fail("isr_stub_table must list every stub exactly once in vector order")

    common_match = re.search(
        r'"isr_common:",(?P<body>.*?)(?:"\.section \.rodata")',
        source,
        re.DOTALL,
    )
    if not common_match:
        fail("isr_common body not found")
    common = common_match.group("body")
    required_common_instructions = [
        '"movq (%rsp), %rdi",',
        '"movq 8(%rsp), %rsi",',
        '"movq 16(%rsp), %rdx",',
        '"andq $-16, %rsp",',
        '"call archiaos_exception_dispatch",',
        '".Larchiaos_exception_halt:",',
        '"cli",',
        '"hlt",',
        '"jmp .Larchiaos_exception_halt",',
    ]
    positions = [common.find(instruction) for instruction in required_common_instructions]
    if any(position < 0 for position in positions):
        fail("common entry is missing an argument, alignment, dispatch, or fatal-halt instruction")
    if positions != sorted(positions):
        fail("common entry instruction order changed; review the stack/ABI contract")

    dispatcher_match = re.search(
        r'pub extern "C" fn archiaos_exception_dispatch\\b(?P<signature>[^\\{]*)\\{',
        source,
        re.DOTALL,
    )
    if not dispatcher_match:
        fail("fatal exception dispatcher not found")
    if not re.search(r'->\\s*!\\s*

if __name__ == "__main__":
    main()
, dispatcher_match.group("signature")):
        fail("exception dispatcher must be non-returning until a safe return path exists")

    install_match = re.search(
        r'pub unsafe fn install\\(\\) \\{(?P<body>.*?)\\n\\}',
        source,
        re.DOTALL,
    )
    if not install_match:
        fail("IDT install routine not found")
    install_body = install_match.group("body")
    if "for vector in 0..IDT_ENTRY_COUNT" not in install_body:
        fail("IDT install must populate every vector entry")
    if 'asm!("lidt [{0}]"' not in install_body:
        fail("IDT install must load the completed descriptor with LIDT")

    print("ISR audit passed: 256 stubs, error-code normalization, common-entry ABI safeguards, IDT installation, and fatal dispatcher contract")


if __name__ == "__main__":
    main()
