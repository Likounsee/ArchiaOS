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
VECTOR_SOURCE = Path(__file__).resolve().parents[1] / "kernel/src/interrupt_vectors.rs"
ERROR_CODE_VECTORS = {8, 10, 11, 12, 13, 14, 17, 21, 29, 30}


def fail(message: str) -> None:
    print(f"ISR audit failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    vector_source = VECTOR_SOURCE.read_text(encoding="utf-8")
    metadata_match = re.search(
        r'pub const fn cpu_exception_has_error_code\(vector: u8\) -> bool \{\s*matches!\(vector, ([^)]*)\)',
        vector_source,
        re.DOTALL,
    )
    if not metadata_match:
        fail("cpu_exception_has_error_code metadata not found")
    metadata_vectors = set()
    for item in metadata_match.group(1).split("|"):
        item = item.strip()
        if "..=" in item:
            lower, upper = (int(value.strip()) for value in item.split("..=", 1))
            metadata_vectors.update(range(lower, upper + 1))
        else:
            metadata_vectors.add(int(item))
    if metadata_vectors != ERROR_CODE_VECTORS:
        fail(
            "ISR error-code list disagrees with cpu_exception_has_error_code: "
            f"stubs={sorted(ERROR_CODE_VECTORS)}, metadata={sorted(metadata_vectors)}"
        )

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
        r'pub extern "C" fn archiaos_exception_dispatch\b(?P<signature>[^{]*)\{',
        source,
        re.DOTALL,
    )
    if not dispatcher_match:
        fail("fatal exception dispatcher not found")
    if not re.search(r'->\s*!\s*$', dispatcher_match.group("signature")):
        fail("exception dispatcher must be non-returning until a safe return path exists")

    install_match = re.search(
        r'pub unsafe fn install\(\) \{(?P<body>.*?)\n\}',
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

    gate_match = re.search(r'const INTERRUPT_GATE: u8 = (0x[0-9a-fA-F]+);', source)
    if not gate_match or int(gate_match.group(1), 16) != 0x8e:
        fail("IDT entries must use the present, ring-0 interrupt-gate attribute 0x8e")

    gate_match = re.search(
        r'fn interrupt_gate\(handler: u64, selector: u16\) -> Self \{(?P<body>.*?)\n    \}',
        source,
        re.DOTALL,
    )
    if not gate_match:
        fail("IDT interrupt-gate constructor not found")
    gate_body = gate_match.group("body")
    for field in (
        "split_x86_64_idt_handler_address(handler)",
        "selector,",
        "ist: 0,",
        "attributes: INTERRUPT_GATE,",
        "reserved: 0,",
    ):
        if field not in gate_body:
            fail(f"IDT gate constructor is missing required field/invariant: {field}")

    descriptor_match = re.search(
        r'let descriptor = Idtr \{(?P<body>.*?)\n    \};',
        install_body,
        re.DOTALL,
    )
    if not descriptor_match:
        fail("IDTR descriptor construction not found")
    descriptor_body = descriptor_match.group("body")
    if "core::mem::size_of::<IdtEntry>() * IDT_ENTRY_COUNT - 1" not in descriptor_body:
        fail("IDTR limit must cover all 256 IDT entries")
    if "core::ptr::addr_of!(IDT) as u64" not in descriptor_body:
        fail("IDTR base must point to the initialized IDT")

    print("ISR audit passed: 256 stubs, error-code normalization, common-entry ABI safeguards, IDT gate layout, IDTR coverage, installation, and fatal dispatcher contract")


if __name__ == "__main__":
    main()
