#![no_std]
#![no_main]
use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use core::ptr::read_volatile;
const BOOT_INFO_MAGIC:u64=0x4942_4149_4843_5241;
const BOOT_INFO_VERSION:u32=1;
const COM1:u16=0x3f8;
const MIN_DESC:u64=40;
#[repr(C)]
struct BootInfo {
 magic:u64, version:u32, struct_size:u32, kernel_entry:u64,
 memory_map:*const u8, memory_map_size:u64, memory_map_descriptor_size:u64,
 memory_map_descriptor_version:u32, flags:u32, acpi_rsdp:u64, device_tree:u64,
 framebuffer_base:u64, framebuffer_size:u64,
}
global_asm!(
 ".section .text.boot, \"ax\"", ".global _start", ".type _start, @function", "_start:",
 "cli", "cld", "lea rsp, [rip + __stack_top]", "and rsp, -16",
 "call rust_kernel_main", "1:", "cli", "hlt", "jmp 1b",
 ".section .bss.stack, \"aw\", @nobits", ".balign 4096",
 ".global __stack_bottom", "__stack_bottom:", ".skip 65536",
 ".global __stack_top", "__stack_top:",
);
struct Serial;
impl Serial {
 fn init(&mut self){unsafe{port_out(COM1+1,0);port_out(COM1+3,0x80);port_out(COM1,3);port_out(COM1+1,0);port_out(COM1+3,3);port_out(COM1+2,0xc7);port_out(COM1+4,0x0b);}}
 fn byte(&mut self,b:u8){for _ in 0..100_000{if unsafe{port_in(COM1+5)}&0x20!=0{unsafe{port_out(COM1,b);}return;}core::hint::spin_loop();}}
}
impl Write for Serial {fn write_str(&mut self,s:&str)->fmt::Result{for b in s.bytes(){if b==b'\n'{self.byte(b'\r');}self.byte(b);}Ok(())}}
unsafe fn port_out(p:u16,v:u8){unsafe{asm!("out dx, al",in("dx")p,in("al")v,options(nomem,nostack,preserves_flags));}}
unsafe fn port_in(p:u16)->u8{let v:u8;unsafe{asm!("in al, dx",in("dx")p,out("al")v,options(nomem,nostack,preserves_flags));}v}
#[no_mangle]
pub extern "C" fn rust_kernel_main(info:*const BootInfo)->!{
 let mut serial=Serial;serial.init();let _=writeln!(serial,"ArchiaOS UEFI test kernel: handoff received");
 if info.is_null(){let _=writeln!(serial,"ArchiaOS UEFI test kernel: BootInfo pointer missing");halt();}
 let b=unsafe{&*info};
 if b.magic!=BOOT_INFO_MAGIC||b.version!=BOOT_INFO_VERSION||b.struct_size!=core::mem::size_of::<BootInfo>() as u32||b.kernel_entry==0||b.memory_map.is_null()||b.memory_map_size==0||b.memory_map_descriptor_size<MIN_DESC||b.memory_map_descriptor_size>4096||b.memory_map_size%b.memory_map_descriptor_size!=0||b.memory_map_size>1024*1024{
  let _=writeln!(serial,"ArchiaOS UEFI test kernel: BootInfo validation failed");halt();
 }
 let desc_size=b.memory_map_descriptor_size as usize;
 let count=(b.memory_map_size/b.memory_map_descriptor_size) as usize;
 if count==0||count>65536{let _=writeln!(serial,"ArchiaOS UEFI test kernel: map entry count rejected");halt();}
 for i in 0..count {let d=unsafe{b.memory_map.add(i*desc_size)};let _kind=unsafe{read_volatile(d.cast::<u32>())};}
 let _=writeln!(serial,"ArchiaOS UEFI test kernel: BootInfo validated");
 let _=writeln!(serial,"ArchiaOS UEFI test kernel: memory map descriptors: {}",count);
 let _=writeln!(serial,"ArchiaOS UEFI test kernel: identity-mapped handoff passed");
 halt()
}
fn halt()->!{loop{unsafe{asm!("cli; hlt",options(nomem,nostack));}}}
#[panic_handler]
fn panic(_:&PanicInfo<'_>)->!{let mut s=Serial;s.init();let _=writeln!(s,"ArchiaOS UEFI test kernel: PANIC");halt()}
