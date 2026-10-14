//! Bootstrap x86-64 kernel virtual-memory mappings for allocated physical frames.
//!
//! The UEFI loader identity-maps ordinary RAM below 4 GiB. This module creates
//! an on-demand kernel alias in a separate PML4 slot, allowing RAM above 4 GiB
//! to be accessed without treating a physical address as a virtual pointer.
//! Page-table pages are kept below 4 GiB until recursive/high-table access exists.

use core::arch::asm;
use core::fmt::Write;
use core::ptr;

use crate::memory;

pub const PHYSICAL_MAP_BASE: u64 = 0xffff_9000_0000_0000;
const PAGE_SIZE: u64 = 4096;
const FOUR_GIB: u64 = 1u64 << 32;
const TRACKED_PHYSICAL_LIMIT: u64 = 1u64 << 39;
const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const LARGE_PAGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmmError {
    InvalidPhysicalAddress,
    InvalidVirtualAddress,
    InvalidPermissions,
    PageNotAllocated,
    OutOfPageTables,
    PageTableOutsideIdentityMap,
    LargePageConflict,
    AlreadyMapped,
    NotMapped,
    MappingConflict,
    MemoryVerificationFailed,
    FreeFailed,
}

fn is_canonical(address: u64) -> bool {
    let upper = address >> 48;
    if (address >> 47) & 1 == 0 { upper == 0 } else { upper == 0xffff }
}

fn virtual_address(physical_address: u64) -> Result<u64, VmmError> {
    if physical_address == 0
        || physical_address % PAGE_SIZE != 0
        || physical_address >= TRACKED_PHYSICAL_LIMIT
    {
        return Err(VmmError::InvalidPhysicalAddress);
    }
    let address = PHYSICAL_MAP_BASE.checked_add(physical_address)
        .ok_or(VmmError::InvalidVirtualAddress)?;
    if !is_canonical(address) {
        return Err(VmmError::InvalidVirtualAddress);
    }
    Ok(address)
}

unsafe fn table_pointer(entry: u64) -> Result<*mut u64, VmmError> {
    let physical = entry & ADDRESS_MASK;
    if physical >= FOUR_GIB {
        return Err(VmmError::PageTableOutsideIdentityMap);
    }
    Ok(physical as *mut u64)
}

unsafe fn ensure_table(entry_pointer: *mut u64, reject_large: bool) -> Result<*mut u64, VmmError> {
    let entry = unsafe { ptr::read_volatile(entry_pointer) };
    if entry & PRESENT == 0 {
        let physical = memory::alloc_page_below_4g().ok_or(VmmError::OutOfPageTables)?;
        let table = physical as *mut u64;
        unsafe {
            ptr::write_bytes(table.cast::<u8>(), 0, PAGE_SIZE as usize);
            ptr::write_volatile(entry_pointer, physical | PRESENT | WRITABLE);
        }
        return Ok(table);
    }
    if reject_large && entry & LARGE_PAGE != 0 {
        return Err(VmmError::LargePageConflict);
    }
    unsafe { table_pointer(entry) }
}

/// Map an allocated physical frame into a kernel-only NX, writable virtual alias.
/// The physical frame must remain allocated until its alias is removed.
pub fn map_physical_page(physical_address: u64) -> Result<*mut u8, VmmError> {
    let address = virtual_address(physical_address)?;
    if !memory::is_allocated_page(physical_address) {
        return Err(VmmError::PageNotAllocated);
    }

    unsafe {
        let mut cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        let root = cr3 & ADDRESS_MASK;
        if root >= FOUR_GIB {
            return Err(VmmError::PageTableOutsideIdentityMap);
        }
        let pml4 = root as *mut u64;
        let pml4_index = ((address >> 39) & 0x1ff) as usize;
        let pdpt_index = ((address >> 30) & 0x1ff) as usize;
        let pd_index = ((address >> 21) & 0x1ff) as usize;
        let pt_index = ((address >> 12) & 0x1ff) as usize;

        let pdpt = ensure_table(pml4.add(pml4_index), false)?;
        let pd = ensure_table(pdpt.add(pdpt_index), true)?;
        let pt = ensure_table(pd.add(pd_index), true)?;
        let pte = pt.add(pt_index);
        if ptr::read_volatile(pte) & PRESENT != 0 {
            return Err(VmmError::AlreadyMapped);
        }
        ptr::write_volatile(pte, physical_address | PRESENT | WRITABLE | NO_EXECUTE);
        asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
    }
    Ok(address as *mut u8)
}

/// Remove a physical alias through the common page-table cleanup path.
pub fn unmap_physical_page(physical_address: u64) -> Result<(), VmmError> {
    let address = virtual_address(physical_address)?;
    unmap_kernel_page(address, physical_address)
}

/// Map a physical frame at a caller-selected kernel virtual address.
///
/// User-space addresses are rejected and writable+executable mappings are
/// forbidden. This bootstrap mapper is single-CPU only; it does not yet perform
/// TLB shootdowns on other CPUs or manage per-process address spaces.
pub fn map_kernel_page(
    target_virtual_address: u64,
    physical_address: u64,
    writable: bool,
    executable: bool,
) -> Result<(), VmmError> {
    if target_virtual_address < 0xffff_8000_0000_0000
        || !is_canonical(target_virtual_address)
        || target_virtual_address % PAGE_SIZE != 0
    {
        return Err(VmmError::InvalidVirtualAddress);
    }
    if writable && executable {
        return Err(VmmError::InvalidPermissions);
    }
    if physical_address == 0
        || physical_address % PAGE_SIZE != 0
        || physical_address >= TRACKED_PHYSICAL_LIMIT
    {
        return Err(VmmError::InvalidPhysicalAddress);
    }
    if !memory::is_allocated_page(physical_address) {
        return Err(VmmError::PageNotAllocated);
    }

    unsafe {
        let mut cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        let root = cr3 & ADDRESS_MASK;
        if root >= FOUR_GIB {
            return Err(VmmError::PageTableOutsideIdentityMap);
        }
        let pml4 = root as *mut u64;
        let pml4_index = ((target_virtual_address >> 39) & 0x1ff) as usize;
        let pdpt_index = ((target_virtual_address >> 30) & 0x1ff) as usize;
        let pd_index = ((target_virtual_address >> 21) & 0x1ff) as usize;
        let pt_index = ((target_virtual_address >> 12) & 0x1ff) as usize;

        let pdpt = ensure_table(pml4.add(pml4_index), false)?;
        let pd = ensure_table(pdpt.add(pdpt_index), true)?;
        let pt = ensure_table(pd.add(pd_index), true)?;
        let pte = pt.add(pt_index);
        if ptr::read_volatile(pte) & PRESENT != 0 {
            return Err(VmmError::AlreadyMapped);
        }
        let mut flags = PRESENT;
        if writable { flags |= WRITABLE; }
        if !executable { flags |= NO_EXECUTE; }
        ptr::write_volatile(pte, physical_address | flags);
        asm!("invlpg [{}]", in(reg) target_virtual_address, options(nostack, preserves_flags));
    }
    Ok(())
}

unsafe fn table_is_empty(table: *mut u64) -> bool {
    for index in 0..512usize {
        if unsafe { ptr::read_volatile(table.add(index)) } & PRESENT != 0 {
            return false;
        }
    }
    true
}

/// Remove a kernel mapping only if its leaf still references the expected frame.
pub fn unmap_kernel_page(
    target_virtual_address: u64,
    expected_physical_address: u64,
) -> Result<(), VmmError> {
    if target_virtual_address < 0xffff_8000_0000_0000
        || !is_canonical(target_virtual_address)
        || target_virtual_address % PAGE_SIZE != 0
    {
        return Err(VmmError::InvalidVirtualAddress);
    }
    if expected_physical_address == 0
        || expected_physical_address % PAGE_SIZE != 0
        || expected_physical_address >= TRACKED_PHYSICAL_LIMIT
    {
        return Err(VmmError::InvalidPhysicalAddress);
    }
    unsafe {
        let mut cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        let root = cr3 & ADDRESS_MASK;
        if root >= FOUR_GIB {
            return Err(VmmError::PageTableOutsideIdentityMap);
        }
        let pml4 = root as *mut u64;
        let pml4_index = ((target_virtual_address >> 39) & 0x1ff) as usize;
        let pdpt_index = ((target_virtual_address >> 30) & 0x1ff) as usize;
        let pd_index = ((target_virtual_address >> 21) & 0x1ff) as usize;
        let pt_index = ((target_virtual_address >> 12) & 0x1ff) as usize;

        let pml4e = ptr::read_volatile(pml4.add(pml4_index));
        if pml4e & PRESENT == 0 { return Err(VmmError::NotMapped); }
        let pdpt = table_pointer(pml4e)?;
        let pdpte = ptr::read_volatile(pdpt.add(pdpt_index));
        if pdpte & PRESENT == 0 { return Err(VmmError::NotMapped); }
        if pdpte & LARGE_PAGE != 0 { return Err(VmmError::LargePageConflict); }
        let pd = table_pointer(pdpte)?;
        let pde = ptr::read_volatile(pd.add(pd_index));
        if pde & PRESENT == 0 { return Err(VmmError::NotMapped); }
        if pde & LARGE_PAGE != 0 { return Err(VmmError::LargePageConflict); }
        let pt = table_pointer(pde)?;
        let pte = pt.add(pt_index);
        let leaf = ptr::read_volatile(pte);
        if leaf & PRESENT == 0 { return Err(VmmError::NotMapped); }
        if leaf & ADDRESS_MASK != expected_physical_address {
            return Err(VmmError::MappingConflict);
        }
        let pt_physical = pde & ADDRESS_MASK;
        let pd_physical = pdpte & ADDRESS_MASK;
        let pdpt_physical = pml4e & ADDRESS_MASK;
        ptr::write_volatile(pte, 0);
        asm!("invlpg [{}]", in(reg) target_virtual_address, options(nostack, preserves_flags));

        // Reclaim only empty tables that the PMM confirms are allocated. UEFI-owned
        // tables are reserved and must never be returned to the physical allocator.
        if memory::is_allocated_page(pt_physical) && table_is_empty(pt) {
            ptr::write_volatile(pd.add(pd_index), 0);
            if !memory::free_page(pt_physical) {
                return Err(VmmError::FreeFailed);
            }
            if memory::is_allocated_page(pd_physical) && table_is_empty(pd) {
                ptr::write_volatile(pdpt.add(pdpt_index), 0);
                if !memory::free_page(pd_physical) {
                    return Err(VmmError::FreeFailed);
                }
                if memory::is_allocated_page(pdpt_physical) && table_is_empty(pdpt) {
                    ptr::write_volatile(pml4.add(pml4_index), 0);
                    if !memory::free_page(pdpt_physical) {
                        return Err(VmmError::FreeFailed);
                    }
                }
            }
        }
    }
    Ok(())
}

fn test_general_mapping<W: Write>(log: &mut W) -> Result<(), VmmError> {
    const TEST_VIRTUAL_ADDRESS: u64 = 0xffff_a000_0000_0000;
    let physical = memory::alloc_page_below_4g().ok_or(VmmError::OutOfPageTables)?;
    if map_kernel_page(TEST_VIRTUAL_ADDRESS, physical, true, true)
        != Err(VmmError::InvalidPermissions)
    {
        let _ = memory::free_page(physical);
        return Err(VmmError::InvalidPermissions);
    }
    map_kernel_page(TEST_VIRTUAL_ADDRESS, physical, true, false)?;
    let word = TEST_VIRTUAL_ADDRESS as *mut u64;
    let expected = 0x6b65_726e_656c_7631u64;
    unsafe { ptr::write_volatile(word, expected) };
    let actual = unsafe { ptr::read_volatile(word) };
    let unmapped = unmap_kernel_page(TEST_VIRTUAL_ADDRESS, physical);
    if actual != expected {
        let _ = unmapped;
        let _ = memory::free_page(physical);
        return Err(VmmError::MemoryVerificationFailed);
    }
    unmapped?;
    if !memory::free_page(physical) {
        return Err(VmmError::FreeFailed);
    }
    let _ = writeln!(log, "VMM: arbitrary kernel mapping, W^X rejection, access and unmap passed");
    Ok(())
}

fn test_one<W: Write>(log: &mut W, physical_address: u64) -> Result<(), VmmError> {
    let alias = map_physical_page(physical_address)?;
    let word = alias.cast::<u64>();
    let expected = 0xa11c_e005_5eed_0000u64 ^ physical_address;
    unsafe { ptr::write_volatile(word, expected) };
    let actual = unsafe { ptr::read_volatile(word) };
    let unmapped = unmap_physical_page(physical_address);
    if actual != expected {
        let _ = unmapped;
        return Err(VmmError::MemoryVerificationFailed);
    }
    unmapped?;
    if !memory::free_page(physical_address) {
        return Err(VmmError::FreeFailed);
    }
    let _ = writeln!(log, "VMM: mapped, wrote, read, unmapped and freed frame {:#x}", physical_address);
    Ok(())
}

/// Boot-time integration test against the active CR3, including high RAM if available.
pub fn self_test<W: Write>(log: &mut W) -> Result<(), VmmError> {
    let free_before = memory::free_page_count();
    test_general_mapping(log)?;
    let low = memory::alloc_page_below_4g().ok_or(VmmError::OutOfPageTables)?;
    test_one(log, low)?;
    match memory::alloc_page_above_4g() {
        Some(high) => {
            test_one(log, high)?;
            let _ = writeln!(log, "VMM: explicit virtual access to physical RAM above 4 GiB passed");
        }
        None => {
            let _ = writeln!(log, "VMM: no allocatable RAM above 4 GiB in this boot; high-memory path not exercised");
        }
    }
    if memory::free_page_count() != free_before {
        return Err(VmmError::FreeFailed);
    }
    let _ = writeln!(log, "VMM: physical frame and intermediate page-table counts restored after unmap");
    Ok(())
}
