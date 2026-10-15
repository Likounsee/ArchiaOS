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
MAIN_SOURCE = Path(__file__).resolve().parents[1] / "kernel/src/main.rs"
ERROR_CODE_VECTORS = {8, 10, 11, 12, 13, 14, 17, 21, 29, 30}


def fail(message: str) -> None:
    print(f"ISR audit failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    vector_source = VECTOR_SOURCE.read_text(encoding="utf-8")
    main_source = MAIN_SOURCE.read_text(encoding="utf-8")
    count_match = re.search(r'const IDT_ENTRY_COUNT: usize = (\d+);', source)
    if not count_match or int(count_match.group(1)) != 256:
        fail("IDT_ENTRY_COUNT must remain exactly 256")
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
    stub_vectors = [int(match.group(1)) for match in stubs]
    if len(stub_vectors) != 256:
        fail(f"expected exactly 256 ISR stub definitions, found {len(stub_vectors)}")
    if len(set(stub_vectors)) != len(stub_vectors):
        duplicates = sorted(
            vector for vector in set(stub_vectors) if stub_vectors.count(vector) > 1
        )
        fail(f"duplicate ISR stub definitions: {duplicates}")

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
        main_source,
        re.DOTALL,
    )
    if not dispatcher_match:
        fail("fatal exception dispatcher not found in kernel/src/main.rs")
    signature = dispatcher_match.group("signature")
    expected_parameters = (
        r'vector:\s*u64',
        r'error_code:\s*u64',
        r'instruction_pointer:\s*u64',
    )
    if not all(re.search(parameter, signature) for parameter in expected_parameters):
        fail("exception dispatcher ABI must accept vector, error code, and instruction pointer as u64")
    if not re.search(r'->\s*!\s*$', signature):
        fail("exception dispatcher must be non-returning until a safe return path exists")

    install_match = re.search(
        r'pub unsafe fn install\(\) \{(?P<body>.*?)\n\}',
        source,
        re.DOTALL,
    )
    if not install_match:
        fail("IDT install routine not found")
    install_body = install_match.group("body")
    population = install_body.find("for vector in 0..IDT_ENTRY_COUNT")
    descriptor_position = install_body.find("let descriptor = Idtr")
    lidt_position = install_body.find('asm!("lidt [{0}]"')
    if population < 0:
        fail("IDT install must populate every vector entry")
    if descriptor_position < 0:
        fail("IDT install must construct the IDTR descriptor")
    if lidt_position < 0:
        fail("IDT install must load the completed descriptor with LIDT")
    if not population < descriptor_position < lidt_position:
        fail("IDT entries must be populated before constructing and loading the IDTR")

    if 'asm!("mov {0:x}, cs"' not in install_body:
        fail("IDT install must obtain the current code-segment selector from CS")
    if "addr_of!(isr_stub_table)" not in install_body or ".add(vector).read()" not in install_body:
        fail("IDT install must read each handler from the bounded ISR stub table")
    if "addr_of_mut!(IDT)" not in install_body or ".add(vector).write(entry)" not in install_body:
        fail("IDT install must write each constructed gate into the bounded IDT")

    expected_layouts = {
        "IdtEntry": [
            ("offset_low", "u16"),
            ("selector", "u16"),
            ("ist", "u8"),
            ("attributes", "u8"),
            ("offset_middle", "u16"),
            ("offset_high", "u32"),
            ("reserved", "u32"),
        ],
        "Idtr": [("limit", "u16"), ("base", "u64")],
    }
    for struct_name, expected_fields in expected_layouts.items():
        struct_match = re.search(
            rf'#\[repr\(C, packed\)\]\s*(?:#\[[^\]]+\]\s*)*struct {struct_name} \{{(?P<body>.*?)\n\}}',
            source,
            re.DOTALL,
        )
        if not struct_match:
            fail(f"{struct_name} must use repr(C, packed) for the hardware-defined layout")
        fields = re.findall(
            r'^\s*(\w+):\s*(\w+),',
            struct_match.group("body"),
            re.MULTILINE,
        )
        if fields != expected_fields:
            fail(
                f"{struct_name} hardware field order/types changed: "
                f"expected={expected_fields}, got={fields}"
            )

    for assertion in (
        "const _: [(); 16] = [(); core::mem::size_of::<IdtEntry>()];",
        "const _: [(); 10] = [(); core::mem::size_of::<Idtr>()];",
        "const _: [(); IDT_ENTRY_COUNT] = [(); 256];",
    ):
        if assertion not in source:
            fail(f"missing compile-time IDT layout/count assertion: {assertion}")

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
        "offset_low,",
        "offset_middle,",
        "offset_high,",
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

    # The controlled #BP gate is intentionally separate from the fatal vector-3
    # stub. Audit its normalized frame, complete GPR save/restore, and IRETQ path.
    breakpoint_stub = re.search(
        r'"\.global isr_breakpoint_test_stub",\s*"isr_breakpoint_test_stub:",(?P<body>.*?)(?=^\s*"\.global isr_apic_stub_48")',
        source,
        re.MULTILINE | re.DOTALL,
    )
    if not breakpoint_stub:
        fail("dedicated returning #BP self-test stub not found")
    breakpoint_body = breakpoint_stub.group("body")
    breakpoint_instructions = [
        '"pushq $0",',
        '"pushq $3",',
        '"movq %rsp, %rdi",',
        '"andq $-16, %rsp",',
        '"call archiaos_test_interrupt_dispatch",',
        '"movq %rax, %rsp",',
        '"addq $16, %rsp",',
        '"iretq",',
    ]
    breakpoint_positions = [breakpoint_body.find(item) for item in breakpoint_instructions]
    if any(position < 0 for position in breakpoint_positions):
        fail("#BP test stub is missing normalized frame, aligned dispatch, stack restoration, or IRETQ")
    if breakpoint_positions != sorted(breakpoint_positions):
        fail("#BP test stub instruction order violates its normalized-frame contract")
    for register in ("rax", "rcx", "rdx", "rbx", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"):
        if f'"pushq %{register}",' not in breakpoint_body or f'"popq %{register}",' not in breakpoint_body:
            fail(f"#BP test stub must preserve %{register}")
    for assertion in (
        "BREAKPOINT_TEST_LAST_ERROR.load(Ordering::SeqCst) == 0",
        "BREAKPOINT_TEST_LAST_RIP.load(Ordering::SeqCst) != 0",
        "BREAKPOINT_TEST_LAST_CS.load(Ordering::SeqCst) != 0",
        "BREAKPOINT_TEST_LAST_RFLAGS.load(Ordering::SeqCst) & (1 << 9) == 0",
        'asm!("int3");',
    ):
        if assertion not in source:
            fail(f"controlled #BP self-test is missing frame assertion: {assertion}")

    print("ISR audit passed: 256 stubs, error-code normalization, #BP frame/IRETQ preservation, common-entry ABI safeguards, IDT gate layout, IDTR coverage, installation, and fatal dispatcher contract")


if __name__ == "__main__":
    main()
