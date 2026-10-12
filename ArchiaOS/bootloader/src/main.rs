#![no_std]
#![no_main]

// First-party UEFI loader. x86-64 currently supports identity-mapped ELF images
// below 4 GiB. The production high-half kernel remains on Limine until its page
// tables and independent memory-bootstrap contract are implemented.
use core::ffi::c_void;
use core::panic::PanicInfo;
use core::ptr::{self, read_unaligned, read_volatile, write_volatile};
use archiaos_bootloader::bootinfo::{BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
use archiaos_bootloader::elf::{load_segment, parse_elf64, ElfError, ElfFile};

type Status = usize;
type Handle = *mut c_void;
const EFI_SUCCESS: Status = 0;
const EFI_ERROR_BIT: Status = 1usize << (usize::BITS - 1);
const EFI_LOAD_ERROR: Status = EFI_ERROR_BIT | 1;
const EFI_INVALID_PARAMETER: Status = EFI_ERROR_BIT | 2;
const EFI_UNSUPPORTED: Status = EFI_ERROR_BIT | 3;
const EFI_BAD_BUFFER_SIZE: Status = EFI_ERROR_BIT | 4;
const EFI_BUFFER_TOO_SMALL: Status = EFI_ERROR_BIT | 5;
const EFI_NOT_FOUND: Status = EFI_ERROR_BIT | 14;
const EFI_LOADER_CODE: u32 = 1;
const EFI_LOADER_DATA: u32 = 2;
const EFI_ALLOCATE_ADDRESS: u32 = 2;
const EFI_FILE_MODE_READ: u64 = 1;
const MAX_KERNEL_IMAGE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_KERNEL_MEMORY_SIZE: u64 = 128 * 1024 * 1024;
const MAX_MEMORY_MAP_SIZE: usize = 1024 * 1024;
const MAX_ELF_PROGRAM_HEADERS: usize = 1024;
const PAGE_SIZE: u64 = 4096;
const LOWEST_KERNEL_ADDRESS: u64 = 1024 * 1024;
const IDENTITY_LIMIT: u64 = 1u64 << 32;
const EFI_MEMORY_DESCRIPTOR_MIN_SIZE: usize = 40;
const ELF_PF_X: u32 = 1;
const ELF_MACHINE: u16 = if cfg!(target_arch = "x86_64") { 62 } else { 183 };

#[repr(C)]
struct TableHeader { signature: u64, revision: u32, header_size: u32, crc32: u32, reserved: u32 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid { data1: u32, data2: u16, data3: u16, data4: [u8; 8] }
const LOADED_IMAGE_GUID: Guid = Guid { data1:0x5b1b31a1, data2:0x9562, data3:0x11d2, data4:[0x8e,0x3f,0x00,0xa0,0xc9,0x69,0x72,0x3b] };
const SIMPLE_FILE_SYSTEM_GUID: Guid = Guid { data1:0x964e5b22, data2:0x6459, data3:0x11d2, data4:[0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b] };
const FILE_INFO_GUID: Guid = Guid { data1:0x09576e92, data2:0x6d3f, data3:0x11d2, data4:[0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b] };
const ACPI_20_TABLE_GUID: Guid = Guid { data1:0x8868e871, data2:0xe4f1, data3:0x11d3, data4:[0xbc,0x22,0x00,0x80,0xc7,0x3c,0x88,0x81] };
const ACPI_10_TABLE_GUID: Guid = Guid { data1:0xeb9d2d30, data2:0x2d88, data3:0x11d3, data4:[0x9a,0x16,0x00,0x90,0x27,0x3f,0xc1,0x4d] };
const DEVICE_TREE_TABLE_GUID: Guid = Guid { data1:0xb1b621d5, data2:0xf19c, data3:0x41a5, data4:[0x83,0x0b,0xd9,0x15,0x2c,0x69,0xaa,0xe0] };

#[repr(C)]
struct SimpleTextOutputProtocol {
    reset: usize,
    output_string: Option<unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> Status>,
    test_string: usize, query_mode: usize, set_mode: usize, set_attribute: usize,
    clear_screen: usize, set_cursor_position: usize, enable_cursor: usize, mode: *mut c_void,
}
type HandleProtocolFn = unsafe extern "efiapi" fn(Handle, *const Guid, *mut *mut c_void) -> Status;
type AllocatePoolFn = unsafe extern "efiapi" fn(u32, usize, *mut *mut c_void) -> Status;
type FreePoolFn = unsafe extern "efiapi" fn(*mut c_void) -> Status;
type AllocatePagesFn = unsafe extern "efiapi" fn(u32, u32, usize, *mut u64) -> Status;
type FreePagesFn = unsafe extern "efiapi" fn(u64, usize) -> Status;
type GetMemoryMapFn = unsafe extern "efiapi" fn(*mut usize, *mut c_void, *mut usize, *mut usize, *mut u32) -> Status;
type ExitBootServicesFn = unsafe extern "efiapi" fn(Handle, usize) -> Status;

#[repr(C)]
struct BootServices {
    header: TableHeader,
    raise_tpl: usize, restore_tpl: usize,
    allocate_pages: Option<AllocatePagesFn>,
    free_pages: Option<FreePagesFn>,
    get_memory_map: Option<GetMemoryMapFn>,
    allocate_pool: Option<AllocatePoolFn>,
    free_pool: Option<FreePoolFn>,
    create_event: usize, set_timer: usize, wait_for_event: usize, signal_event: usize,
    close_event: usize, check_event: usize, install_protocol_interface: usize,
    reinstall_protocol_interface: usize, uninstall_protocol_interface: usize,
    handle_protocol: Option<HandleProtocolFn>,
    reserved: usize, register_protocol_notify: usize, locate_handle: usize,
    locate_device_path: usize, install_configuration_table: usize, load_image: usize,
    start_image: usize, exit: usize, unload_image: usize,
    exit_boot_services: Option<ExitBootServicesFn>,
    get_next_monotonic_count: usize, stall: usize, set_watchdog_timer: usize,
    connect_controller: usize, disconnect_controller: usize, open_protocol: usize,
    close_protocol: usize, open_protocol_information: usize, protocols_per_handle: usize,
    locate_handle_buffer: usize, locate_protocol: usize,
    install_multiple_protocol_interfaces: usize, uninstall_multiple_protocol_interfaces: usize,
    calculate_crc32: usize, copy_mem: usize, set_mem: usize, create_event_ex: usize,
}
#[repr(C)]
struct SystemTable {
    header: TableHeader, firmware_vendor: *mut u16, firmware_revision: u32, _padding: u32,
    console_in_handle: Handle, con_in: *mut c_void,
    console_out_handle: Handle, con_out: *mut SimpleTextOutputProtocol,
    standard_error_handle: Handle, std_err: *mut SimpleTextOutputProtocol,
    runtime_services: *mut c_void, boot_services: *mut BootServices,
    number_of_table_entries: usize, configuration_table: *mut c_void,
}
#[repr(C)]
struct ConfigurationTable {
    vendor_guid: Guid,
    vendor_table: *mut c_void,
}

#[repr(C)]
struct LoadedImageProtocol {
    revision: u32, _padding: u32, parent_handle: Handle, system_table: *mut SystemTable,
    device_handle: Handle, file_path: *mut c_void, reserved: *mut c_void,
    load_options_size: u32, _padding2: u32, load_options: *mut c_void,
    image_base: *mut c_void, image_size: u64, image_code_type: u32,
    image_data_type: u32, unload: usize,
}
#[repr(C)]
struct SimpleFileSystemProtocol {
    revision: u64,
    open_volume: Option<unsafe extern "efiapi" fn(*mut SimpleFileSystemProtocol, *mut *mut FileProtocol) -> Status>,
}
#[repr(C)]
struct FileProtocol {
    revision: u64,
    open: Option<unsafe extern "efiapi" fn(*mut FileProtocol, *mut *mut FileProtocol, *const u16, u64, u64) -> Status>,
    close: Option<unsafe extern "efiapi" fn(*mut FileProtocol) -> Status>,
    delete: usize,
    read: Option<unsafe extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut c_void) -> Status>,
    write: usize, get_position: usize, set_position: usize,
    get_info: Option<unsafe extern "efiapi" fn(*mut FileProtocol, *const Guid, *mut usize, *mut c_void) -> Status>,
    set_info: usize, flush: usize,
}

const fn ascii_to_utf16<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut out = [0u16; N]; let mut i = 0;
    while i < N { out[i] = bytes[i] as u16; i += 1; }
    out
}
static BANNER: [u16; 42] = ascii_to_utf16(b"ArchiaOS Bootloader: UEFI entry reached\r\n\0");
static STAGE_FS: [u16; 48] = ascii_to_utf16(b"ArchiaOS Bootloader: opening EFI kernel image\r\n\0");
static STAGE_ELF: [u16; 47] = ascii_to_utf16(b"ArchiaOS Bootloader: validating ELF64 kernel\r\n\0");
static STAGE_LOADING: [u16; 60] = ascii_to_utf16(b"ArchiaOS Bootloader: loading identity-mapped ELF segments\r\n\0");
static STAGE_EXIT: [u16; 50] = ascii_to_utf16(b"ArchiaOS Bootloader: exiting UEFI boot services\r\n\0");
static FAIL_SERVICES: [u16; 58] = ascii_to_utf16(b"ArchiaOS Bootloader: required UEFI boot service missing\r\n\0");
static FAIL_FS: [u16; 53] = ascii_to_utf16(b"ArchiaOS Bootloader: cannot open EFI system volume\r\n\0");
static FAIL_FILE: [u16; 59] = ascii_to_utf16(b"ArchiaOS Bootloader: cannot read EFI/ArchiaOS/kernel.elf\r\n\0");
static FAIL_SIZE: [u16; 50] = ascii_to_utf16(b"ArchiaOS Bootloader: kernel image size rejected\r\n\0");
static FAIL_ELF: [u16; 50] = ascii_to_utf16(b"ArchiaOS Bootloader: invalid ELF64 kernel image\r\n\0");
static FAIL_HANDOFF: [u16; 63] = ascii_to_utf16(b"ArchiaOS Bootloader: image needs unsupported virtual mapping\r\n\0");
static FAIL_SEGMENT: [u16; 63] = ascii_to_utf16(b"ArchiaOS Bootloader: could not load identity-mapped segments\r\n\0");
static FAIL_EXIT: [u16; 47] = ascii_to_utf16(b"ArchiaOS Bootloader: ExitBootServices failed\r\n\0");
static KERNEL_PATH: [u16; 25] = ascii_to_utf16(b"\\EFI\\ArchiaOS\\kernel.elf\0");

struct Serial { output: *mut SimpleTextOutputProtocol }
impl Serial {
    unsafe fn print(&mut self, message: *const u16) -> Result<(), Status> {
        #[cfg(target_arch = "x86_64")]
        unsafe { mirror_to_com1(message); }
        if self.output.is_null() { return Err(EFI_INVALID_PARAMETER); }
        let f = unsafe { (*self.output).output_string }.ok_or(EFI_UNSUPPORTED)?;
        let s = unsafe { f(self.output, message) };
        if s == EFI_SUCCESS { Ok(()) } else { Err(s) }
    }
}
#[cfg(target_arch = "x86_64")]
unsafe fn port_out(p:u16,v:u8) { use core::arch::asm; unsafe { asm!("out dx, al",in("dx") p,in("al") v,options(nomem,nostack,preserves_flags)); } }
#[cfg(target_arch = "x86_64")]
unsafe fn port_in(p:u16)->u8 { use core::arch::asm; let v:u8; unsafe { asm!("in al, dx",in("dx") p,out("al") v,options(nomem,nostack,preserves_flags)); } v }
#[cfg(target_arch = "x86_64")]
unsafe fn mirror_to_com1(msg:*const u16) {
    const C:u16=0x3f8;
    unsafe { port_out(C+1,0);port_out(C+3,0x80);port_out(C,3);port_out(C+1,0);port_out(C+3,3);port_out(C+2,0xc7);port_out(C+4,0x0b); }
    for i in 0..256usize {
        let w=unsafe { read_volatile(msg.add(i)) }; if w==0 { break; }
        let b=if w<=0x7f { w as u8 } else { b'?' };
        for _ in 0..100_000 { if unsafe { port_in(C+5) } & 0x20 != 0 { break; } core::hint::spin_loop(); }
        unsafe { port_out(C,b); }
    }
}

#[cfg(target_arch="x86_64")]
unsafe fn emergency_exit_diagnostic() {
    unsafe { mirror_to_com1(FAIL_EXIT.as_ptr()); }
}

#[cfg(target_arch="aarch64")]
unsafe fn emergency_exit_diagnostic() {
    // This emergency path is used only after an ExitBootServices attempt, when
    // UEFI console services may no longer be callable. The ARM64 CI target is
    // QEMU virt, whose PL011 UART is mapped at 0x09000000.
    const BASE: usize = 0x0900_0000;
    const DR: usize = BASE;
    const FR: usize = BASE + 0x18;
    const IBRD: usize = BASE + 0x24;
    const FBRD: usize = BASE + 0x28;
    const LCR_H: usize = BASE + 0x2c;
    const CR: usize = BASE + 0x30;
    const ICR: usize = BASE + 0x44;
    unsafe {
        write_volatile(CR as *mut u32, 0);
        write_volatile(ICR as *mut u32, 0x7ff);
        write_volatile(IBRD as *mut u32, 13);
        write_volatile(FBRD as *mut u32, 1);
        write_volatile(LCR_H as *mut u32, 0x70);
        write_volatile(CR as *mut u32, 0x301);
    }
    for i in 0..256usize {
        let word = unsafe { read_volatile(FAIL_EXIT.as_ptr().add(i)) };
        if word == 0 { break; }
        let byte = if word <= 0x7f { word as u8 } else { b'?' };
        for _ in 0..100_000 {
            if unsafe { read_volatile(FR as *const u32) } & (1 << 5) == 0 { break; }
            core::hint::spin_loop();
        }
        unsafe { write_volatile(DR as *mut u32, byte as u32); }
    }
}

unsafe fn configuration_table_pointer(st:*mut SystemTable,guid:Guid)->u64 {
    if st.is_null() { return 0; }
    let count=unsafe { read_volatile(ptr::addr_of!((*st).number_of_table_entries)) };
    let table=unsafe { read_volatile(ptr::addr_of!((*st).configuration_table)) }.cast::<ConfigurationTable>();
    if table.is_null()||count>4096 { return 0; }
    for index in 0..count {
        let entry=unsafe { &*table.add(index) };
        let candidate=entry.vendor_guid;
        if candidate.data1==guid.data1
            && candidate.data2==guid.data2
            && candidate.data3==guid.data3
            && candidate.data4==guid.data4
            && !entry.vendor_table.is_null()
        {
            return entry.vendor_table as usize as u64;
        }
    }
    0
}

unsafe fn validate_fdt_header(address:u64)->bool {
    use archiaos_bootloader::device_tree::{parse_header, HEADER_SIZE, MAX_TOTAL_SIZE};
    if address==0||address>usize::MAX as u64 { return false; }
    let base=address as usize as *const u8;
    // The EFI Device Tree configuration entry points to a readable FDT blob.
    // Read its advertised total size only after checking the header prefix, then
    // pass a bounded slice to the shared, unit-tested parser.
    let total=unsafe {
        u32::from_be(read_unaligned(base.add(4).cast::<u32>())) as usize
    };
    if total<HEADER_SIZE||total>MAX_TOTAL_SIZE { return false; }
    let image=unsafe { core::slice::from_raw_parts(base,total) };
    parse_header(image).is_ok()
}

unsafe fn validate_rsdp_pointer(address:u64)->bool {
    use archiaos_bootloader::acpi::{
        parse_rsdp, MAX_RSDP_SIZE, RSDP_V1_SIZE, RSDP_V2_MIN_SIZE,
    };
    if address==0||address>usize::MAX as u64 { return false; }
    let base=address as usize as *const u8;
    let revision=unsafe { read_volatile(base.add(15)) };
    let size=if revision<2 {
        RSDP_V1_SIZE
    } else {
        // ACPI revision 2+ defines the length field at byte 20.
        let length=unsafe { u32::from_le(read_unaligned(base.add(20).cast::<u32>())) as usize };
        if length<RSDP_V2_MIN_SIZE||length>MAX_RSDP_SIZE { return false; }
        length
    };
    let image=unsafe { core::slice::from_raw_parts(base,size) };
    parse_rsdp(image).is_ok()
}

unsafe fn protocol<T>(bs:*mut BootServices,h:Handle,g:*const Guid)->Result<*mut T,Status> {
    if bs.is_null()||h.is_null() { return Err(EFI_INVALID_PARAMETER); }
    let f=unsafe { (*bs).handle_protocol }.ok_or(EFI_UNSUPPORTED)?;
    let mut out:*mut c_void=ptr::null_mut();
    let s=unsafe { f(h,g,&mut out) }; if s!=EFI_SUCCESS { return Err(s); }
    if out.is_null() { return Err(EFI_UNSUPPORTED); } Ok(out.cast())
}
unsafe fn allocate_pool(bs:*mut BootServices,n:usize)->Result<*mut c_void,Status> {
    let f=unsafe { (*bs).allocate_pool }.ok_or(EFI_UNSUPPORTED)?;
    let mut out:*mut c_void=ptr::null_mut();
    let s=unsafe { f(EFI_LOADER_DATA,n,&mut out) }; if s!=EFI_SUCCESS { return Err(s); }
    if out.is_null() { return Err(EFI_LOAD_ERROR); } Ok(out)
}
unsafe fn free_pool(bs:*mut BootServices,p:*mut c_void) {
    if !p.is_null() { if let Some(f)=unsafe { (*bs).free_pool } { let _=unsafe { f(p) }; } }
}
unsafe fn close_file(p:*mut FileProtocol) {
    if !p.is_null() { if let Some(f)=unsafe { (*p).close } { let _=unsafe { f(p) }; } }
}
unsafe fn read_kernel_image(handle:Handle,st:*mut SystemTable)->Result<(*mut u8,usize),Status> {
    if st.is_null() { return Err(EFI_INVALID_PARAMETER); }
    let bs=unsafe { (*st).boot_services }; if bs.is_null() { return Err(EFI_UNSUPPORTED); }
    let loaded=unsafe { protocol::<LoadedImageProtocol>(bs,handle,&LOADED_IMAGE_GUID)? };
    let device=unsafe { read_volatile(ptr::addr_of!((*loaded).device_handle)) };
    let fs=unsafe { protocol::<SimpleFileSystemProtocol>(bs,device,&SIMPLE_FILE_SYSTEM_GUID)? };
    let open_volume=unsafe { (*fs).open_volume }.ok_or(EFI_UNSUPPORTED)?;
    let mut root:*mut FileProtocol=ptr::null_mut();
    let s=unsafe { open_volume(fs,&mut root) };
    if s!=EFI_SUCCESS||root.is_null() { unsafe { close_file(root); } return Err(if s==EFI_SUCCESS { EFI_UNSUPPORTED } else { s }); }
    let open=match unsafe { (*root).open } { Some(f)=>f,None=>{unsafe{close_file(root);}return Err(EFI_UNSUPPORTED)} };
    let mut file:*mut FileProtocol=ptr::null_mut();
    let s=unsafe { open(root,&mut file,KERNEL_PATH.as_ptr(),EFI_FILE_MODE_READ,0) };
    if s!=EFI_SUCCESS||file.is_null() { unsafe { close_file(file);close_file(root); } return Err(if s==EFI_SUCCESS {EFI_UNSUPPORTED}else{s}); }
    let info_fn=match unsafe { (*file).get_info } { Some(f)=>f,None=>{unsafe{close_file(file);close_file(root);}return Err(EFI_UNSUPPORTED)} };
    let mut info_size=0usize;
    let s=unsafe { info_fn(file,&FILE_INFO_GUID,&mut info_size,ptr::null_mut()) };
    if s!=EFI_BUFFER_TOO_SMALL||info_size<16||info_size>4096 {
        unsafe { close_file(file);close_file(root); }
        return Err(if s==EFI_SUCCESS||s==EFI_BUFFER_TOO_SMALL {EFI_BAD_BUFFER_SIZE}else{s});
    }
    let info=match unsafe { allocate_pool(bs,info_size) } {
        Ok(p)=>p,Err(e)=>{unsafe{close_file(file);close_file(root);}return Err(e)}
    };
    let cap=info_size;
    let s=unsafe { info_fn(file,&FILE_INFO_GUID,&mut info_size,info) };
    if s!=EFI_SUCCESS||info_size<16||info_size>cap {
        unsafe { free_pool(bs,info);close_file(file);close_file(root); }
        return Err(if s==EFI_SUCCESS {EFI_BAD_BUFFER_SIZE}else{s});
    }
    let file_size=unsafe { read_unaligned((info as *const u8).add(8).cast::<u64>()) };
    unsafe { free_pool(bs,info); }
    if file_size<64||file_size>MAX_KERNEL_IMAGE_SIZE||file_size>usize::MAX as u64 {
        unsafe { close_file(file);close_file(root); } return Err(EFI_BAD_BUFFER_SIZE);
    }
    let buffer=match unsafe { allocate_pool(bs,file_size as usize) } {
        Ok(p)=>p.cast::<u8>(),Err(e)=>{unsafe{close_file(file);close_file(root);}return Err(e)}
    };
    let read=match unsafe { (*file).read } {
        Some(f)=>f,None=>{unsafe{free_pool(bs,buffer.cast());close_file(file);close_file(root);}return Err(EFI_UNSUPPORTED)}
    };
    let expected=file_size as usize;
    let mut actual=0usize;
    while actual<expected {
        let mut chunk=expected-actual;
        let s=unsafe { read(file,&mut chunk,buffer.add(actual).cast()) };
        if s!=EFI_SUCCESS||chunk==0||chunk>expected-actual {
            unsafe { free_pool(bs,buffer.cast());close_file(file);close_file(root); }
            return Err(if s==EFI_SUCCESS {EFI_BAD_BUFFER_SIZE}else{s});
        }
        actual+=chunk;
    }
    unsafe { close_file(file);close_file(root); }
    Ok((buffer,actual))
}

static mut ALLOC_BASES:[u64;MAX_ELF_PROGRAM_HEADERS]=[0;MAX_ELF_PROGRAM_HEADERS];
static mut ALLOC_COUNTS:[usize;MAX_ELF_PROGRAM_HEADERS]=[0;MAX_ELF_PROGRAM_HEADERS];
static mut ALLOC_COUNT:usize=0;

unsafe fn cleanup_segments(bs:*mut BootServices) {
    let free=match unsafe { (*bs).free_pages } {Some(f)=>f,None=>return};
    let count=unsafe { read_volatile(ptr::addr_of!(ALLOC_COUNT)) };
    let bases=ptr::addr_of!(ALLOC_BASES).cast::<u64>();
    let pages=ptr::addr_of!(ALLOC_COUNTS).cast::<usize>();
    for i in (0..count).rev() {
        let b=unsafe { read_volatile(bases.add(i)) }; let n=unsafe { read_volatile(pages.add(i)) };
        let _=unsafe { free(b,n) };
    }
    unsafe { write_volatile(ptr::addr_of_mut!(ALLOC_COUNT),0); }
}
fn physical_page_end(s:&archiaos_bootloader::elf::LoadSegment)->Result<u64,Status> {
    let pages=s.memory_size.checked_add(PAGE_SIZE-1).ok_or(EFI_BAD_BUFFER_SIZE)?/PAGE_SIZE;
    s.physical_address.checked_add(pages.checked_mul(PAGE_SIZE).ok_or(EFI_BAD_BUFFER_SIZE)?).ok_or(EFI_BAD_BUFFER_SIZE)
}

#[cfg(target_arch="aarch64")]
unsafe fn synchronize_instruction_cache(start:u64,size:u64) {
    use core::arch::asm;
    if size==0 { return; }
    let end=match start.checked_add(size) {Some(v)=>v,None=>return};
    let ctr:u64;
    unsafe { asm!("mrs {value}, ctr_el0",value=out(reg) ctr,options(nomem,nostack,preserves_flags)); }
    let dcache_line=4u64<<((ctr>>16)&0xf);
    let icache_line=4u64<<(ctr&0xf);
    let mut address=start&!(dcache_line-1);
    while address<end {
        unsafe { asm!("dc cvau, {address}",address=in(reg) address,options(nostack)); }
        address=match address.checked_add(dcache_line) {Some(v)=>v,None=>break};
    }
    unsafe { asm!("dsb ish",options(nostack,preserves_flags)); }
    address=start&!(icache_line-1);
    while address<end {
        unsafe { asm!("ic ivau, {address}",address=in(reg) address,options(nostack)); }
        address=match address.checked_add(icache_line) {Some(v)=>v,None=>break};
    }
    unsafe { asm!("dsb ish", "isb", options(nostack,preserves_flags)); }
}
unsafe fn load_identity_segments(image:&[u8],elf:&ElfFile,bs:*mut BootServices)->Result<u64,Status> {
    let mut load_count=0usize; let mut total=0u64;
    // Validate every segment address and overlap before any destination write.
    for i in 0..elf.program_header_count() {
        let s=match load_segment(image,elf,i) {Ok(s)=>s,Err(ElfError::NotLoadableSegment)=>continue,Err(_)=>return Err(EFI_LOAD_ERROR)};
        if s.virtual_address!=s.physical_address {return Err(EFI_UNSUPPORTED);}
        if s.physical_address<LOWEST_KERNEL_ADDRESS||s.physical_address%PAGE_SIZE!=0||s.memory_size==0 {return Err(EFI_BAD_BUFFER_SIZE);}
        let end=s.physical_address.checked_add(s.memory_size).ok_or(EFI_BAD_BUFFER_SIZE)?;
        let rounded_end=physical_page_end(&s)?;
        if end>IDENTITY_LIMIT||rounded_end>IDENTITY_LIMIT {return Err(EFI_UNSUPPORTED);}
        total=total.checked_add(s.memory_size).ok_or(EFI_BAD_BUFFER_SIZE)?;
        if total>MAX_KERNEL_MEMORY_SIZE {return Err(EFI_BAD_BUFFER_SIZE);}
        for j in 0..i {
            let other=match load_segment(image,elf,j) {Ok(s)=>s,Err(ElfError::NotLoadableSegment)=>continue,Err(_)=>return Err(EFI_LOAD_ERROR)};
            if s.physical_address<physical_page_end(&other)?&&other.physical_address<rounded_end {return Err(EFI_UNSUPPORTED);}
        }
        load_count+=1;
    }
    if load_count==0||elf.entry()<LOWEST_KERNEL_ADDRESS||elf.entry()>=IDENTITY_LIMIT {return Err(EFI_UNSUPPORTED);}
    let allocate=unsafe { (*bs).allocate_pages }.ok_or(EFI_UNSUPPORTED)?;
    unsafe { write_volatile(ptr::addr_of_mut!(ALLOC_COUNT),0); }
    for i in 0..elf.program_header_count() {
        let s=match load_segment(image,elf,i) {Ok(s)=>s,Err(ElfError::NotLoadableSegment)=>continue,Err(_)=>{unsafe{cleanup_segments(bs);}return Err(EFI_LOAD_ERROR)}};
        let n=((s.memory_size+PAGE_SIZE-1)/PAGE_SIZE) as usize;
        let mut base=s.physical_address;
        let kind=if s.flags&ELF_PF_X!=0 {EFI_LOADER_CODE}else{EFI_LOADER_DATA};
        let status=unsafe { allocate(EFI_ALLOCATE_ADDRESS,kind,n,&mut base) };
        if status!=EFI_SUCCESS||base!=s.physical_address {
            if status==EFI_SUCCESS {if let Some(f)=unsafe{(*bs).free_pages}{let _=unsafe{f(base,n)};}}
            unsafe{cleanup_segments(bs);}
            return Err(if status==EFI_SUCCESS {EFI_LOAD_ERROR}else{status});
        }
        let count=unsafe { read_volatile(ptr::addr_of!(ALLOC_COUNT)) };
        if count>=MAX_ELF_PROGRAM_HEADERS {
            if let Some(f)=unsafe{(*bs).free_pages}{let _=unsafe{f(base,n)};}
            unsafe{cleanup_segments(bs);}
            return Err(EFI_BAD_BUFFER_SIZE);
        }
        let bases=ptr::addr_of_mut!(ALLOC_BASES).cast::<u64>();
        let pages=ptr::addr_of_mut!(ALLOC_COUNTS).cast::<usize>();
        unsafe {write_volatile(bases.add(count),base);write_volatile(pages.add(count),n);write_volatile(ptr::addr_of_mut!(ALLOC_COUNT),count+1);}
        let fo=s.file_offset as usize;let fs=s.file_size as usize;let ms=s.memory_size as usize;
        unsafe {
            ptr::copy_nonoverlapping(image.as_ptr().add(fo),base as *mut u8,fs);
            if ms>fs {ptr::write_bytes((base as *mut u8).add(fs),0,ms-fs);}
        }
        #[cfg(target_arch="aarch64")]
        if s.flags&ELF_PF_X!=0 {
            // The AArch64 I-cache is not guaranteed coherent with writes through
            // the D-cache, so synchronize executable segments before branching.
            unsafe { synchronize_instruction_cache(base,s.memory_size); }
        }
    }
    Ok(elf.entry())
}

#[cfg(any(target_arch="x86_64", target_arch="aarch64"))]
unsafe fn get_boot_info_and_exit(handle:Handle,st:*mut SystemTable,entry:u64)->Result<*mut BootInfo,Status> {
    let bs=unsafe{(*st).boot_services};
    if bs.is_null(){return Err(EFI_UNSUPPORTED);}
    let get_map=unsafe{(*bs).get_memory_map}.ok_or(EFI_UNSUPPORTED)?;
    let exit=unsafe{(*bs).exit_boot_services}.ok_or(EFI_UNSUPPORTED)?;
    // Configuration-table pointers are physical addresses, not runtime service
    // pointers; capture them before relinquishing the UEFI boot environment.
    let mut acpi_rsdp=unsafe { configuration_table_pointer(st,ACPI_20_TABLE_GUID) };
    if acpi_rsdp==0||!unsafe{validate_rsdp_pointer(acpi_rsdp)} {
        acpi_rsdp=unsafe { configuration_table_pointer(st,ACPI_10_TABLE_GUID) };
        if acpi_rsdp!=0&&!unsafe{validate_rsdp_pointer(acpi_rsdp)} {
            acpi_rsdp=0;
        }
    }
    let device_tree=unsafe { configuration_table_pointer(st,DEVICE_TREE_TABLE_GUID) };
    #[cfg(target_arch="aarch64")]
    if device_tree==0||!unsafe{validate_fdt_header(device_tree)} {
        return Err(EFI_NOT_FOUND);
    }
    let info_mem=unsafe{allocate_pool(bs,core::mem::size_of::<BootInfo>())}?;
    let map_mem=match unsafe{allocate_pool(bs,MAX_MEMORY_MAP_SIZE)}{
        Ok(p)=>p,
        Err(e)=>{unsafe{free_pool(bs,info_mem);}return Err(e)}
    };
    let info=info_mem.cast::<BootInfo>();
    let mut exit_was_attempted=false;

    for _ in 0..8 {
        let mut size=MAX_MEMORY_MAP_SIZE;
        let mut key=0usize;
        let mut desc_size=0usize;
        let mut desc_version=0u32;
        let status=unsafe{get_map(&mut size,map_mem,&mut key,&mut desc_size,&mut desc_version)};
        if status!=EFI_SUCCESS {
            if exit_was_attempted {
                // After any failed ExitBootServices attempt, avoid all further
                // services except GetMemoryMap/ExitBootServices. Do not free pools.
                unsafe{emergency_exit_diagnostic();}
                halt_forever();
            }
            unsafe{free_pool(bs,map_mem);free_pool(bs,info_mem);}
            return Err(status);
        }
        if size==0||size>MAX_MEMORY_MAP_SIZE||desc_size<EFI_MEMORY_DESCRIPTOR_MIN_SIZE||size%desc_size!=0 {
            if exit_was_attempted {
                unsafe{emergency_exit_diagnostic();}
                halt_forever();
            }
            unsafe{free_pool(bs,map_mem);free_pool(bs,info_mem);}
            return Err(EFI_BAD_BUFFER_SIZE);
        }

        unsafe{ptr::write(info,BootInfo{
            magic:BOOT_INFO_MAGIC,
            version:BOOT_INFO_VERSION,
            struct_size:core::mem::size_of::<BootInfo>() as u32,
            kernel_entry:entry,
            memory_map:map_mem.cast::<u8>(),
            memory_map_size:size as u64,
            memory_map_descriptor_size:desc_size as u64,
            memory_map_descriptor_version:desc_version,
            flags:0,
            acpi_rsdp,
            device_tree,
            framebuffer_base:0,
            framebuffer_size:0,
        });}

        // Nothing that allocates or invokes another service may intervene here.
        let status=unsafe{exit(handle,key)};
        exit_was_attempted=true;
        if status==EFI_SUCCESS{return Ok(info);}
        if status!=EFI_INVALID_PARAMETER {
            unsafe{emergency_exit_diagnostic();}
            halt_forever();
        }
        // For a stale map key, retry only GetMemoryMap followed immediately
        // by ExitBootServices, retaining all buffer allocations.
    }

    unsafe{emergency_exit_diagnostic();}
    halt_forever()
}
#[cfg(target_arch="x86_64")]
unsafe fn jump_to_kernel(entry:u64,info:*const BootInfo)->! {
    use core::arch::asm;
    unsafe{asm!("cli","cld","jmp rax",in("rax")entry as usize,in("rdi")info as usize,options(noreturn));}
}
#[cfg(target_arch="x86_64")]
fn halt_forever()->! { use core::arch::asm;loop{unsafe{asm!("cli; hlt",options(nomem,nostack));}} }

#[cfg(target_arch="aarch64")]
unsafe fn jump_to_kernel(entry:u64,info:*const BootInfo)->! {
    use core::arch::asm;
    // UEFI AArch64 uses x0 for the first argument. Mask interrupts before handing
    // control to the kernel, preserving the versioned BootInfo pointer in x0.
    unsafe {
        asm!(
            "msr daifset, #0xf",
            "dsb sy",
            "isb",
            "br x16",
            in("x16") entry as usize,
            in("x0") info as usize,
            options(noreturn)
        );
    }
}
#[cfg(target_arch="aarch64")]
fn halt_forever()->! {
    use core::arch::asm;
    loop { unsafe { asm!("msr daifset, #0xf", "wfe", options(nomem,nostack)); } }
}

#[no_mangle]
pub extern "efiapi" fn efi_main(handle:Handle,st:*mut SystemTable)->Status {
    if st.is_null(){return EFI_INVALID_PARAMETER;}
    let mut serial=Serial{output:unsafe{(*st).con_out}};
    if unsafe{serial.print(BANNER.as_ptr())}.is_err(){return EFI_UNSUPPORTED;}
    if unsafe{serial.print(STAGE_FS.as_ptr())}.is_err(){return EFI_UNSUPPORTED;}
    let (image,image_size)=match unsafe{read_kernel_image(handle,st)} {
        Ok(v)=>v,
        Err(status)=>{
            let msg=match status{EFI_BAD_BUFFER_SIZE=>FAIL_SIZE.as_ptr(),EFI_NOT_FOUND=>FAIL_FILE.as_ptr(),EFI_UNSUPPORTED=>FAIL_SERVICES.as_ptr(),_=>FAIL_FS.as_ptr()};
            let _=unsafe{serial.print(msg)};return EFI_LOAD_ERROR;
        }
    };
    let parsed={
        // SAFETY: image is a live UEFI pool allocation of image_size readable bytes.
        let bytes=unsafe{core::slice::from_raw_parts(image,image_size)};
        parse_elf64(bytes,ELF_MACHINE)
    };
    let elf=match parsed {
        Ok(e)=>e,
        Err(_)=>{let bs=unsafe{(*st).boot_services};unsafe{free_pool(bs,image.cast());}let _=unsafe{serial.print(FAIL_ELF.as_ptr())};return EFI_LOAD_ERROR;}
    };
    let _=unsafe{serial.print(STAGE_ELF.as_ptr())};
    let _=unsafe{serial.print(STAGE_LOADING.as_ptr())};
    let bs=unsafe{(*st).boot_services};
    let result={
        let bytes=unsafe{core::slice::from_raw_parts(image,image_size)};
        unsafe{load_identity_segments(bytes,&elf,bs)}
    };
    unsafe{free_pool(bs,image.cast());}
    let entry=match result{
        Ok(e)=>e,
        Err(status)=>{
            unsafe{cleanup_segments(bs);}
            let msg=if status==EFI_UNSUPPORTED{FAIL_HANDOFF.as_ptr()}else{FAIL_SEGMENT.as_ptr()};
            let _=unsafe{serial.print(msg)};
            return status;
        }
    };
    let _=unsafe{serial.print(STAGE_EXIT.as_ptr())};
    let info=match unsafe{get_boot_info_and_exit(handle,st,entry)}{
        Ok(p)=>p,
        Err(status)=>{
            unsafe{cleanup_segments(bs);}
            let msg=if status==EFI_UNSUPPORTED{FAIL_SERVICES.as_ptr()}else if status==EFI_NOT_FOUND||status==EFI_LOAD_ERROR{FAIL_HANDOFF.as_ptr()}else{FAIL_EXIT.as_ptr()};
            let _=unsafe{serial.print(msg)};
            return status;
        }
    };
    // No firmware service is called after ExitBootServices succeeds.
    unsafe{jump_to_kernel(entry,info)}
}
#[panic_handler]
fn panic(_info:&PanicInfo<'_>)->!{loop{core::hint::spin_loop();}}
