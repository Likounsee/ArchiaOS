//! Isolated x86-64 address-space roots with supervisor-only kernel mappings.
//!
//! The initial user virtual region starts at 512 GiB. This intentionally avoids
//! the loader's low identity map, which the bootstrap kernel still needs for
//! physical metadata and page-table access. ELF placement/relocation policy is
//! a separate process-loader task.

use core::arch::asm;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::memory;
use crate::memory_ranges::MAX_PHYSICAL_ADDRESS;

const PAGE_SIZE: u64 = 4096;
const FOUR_GIB: u64 = 1u64 << 32;
const USER_VIRTUAL_BASE: u64 = 1u64 << 39;
const USER_VIRTUAL_LIMIT: u64 = 1u64 << 47;
const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const LARGE_PAGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
static KERNEL_ROOT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpaceError {
    AlreadyInitialized,
    KernelRootUnavailable,
    InvalidKernelRoot,
    OutOfMemory,
    InvalidVirtualAddress,
    InvalidPhysicalAddress,
    InvalidPermissions,
    PageTableOutsideIdentityMap,
    LargePageConflict,
    PageNotAllocated,
    AlreadyMapped,
    NotMapped,
    MappingConflict,
    ActiveAddressSpace,
    AddressSpaceBusy,
    Destroyed,
    FreeFailed,
    VerificationFailed,
}

pub struct AddressSpace {
    root_physical: u64,
    identity_pdpt_physical: u64,
}

fn user_address_is_valid(address: u64) -> bool {
    address >= USER_VIRTUAL_BASE
        && address < USER_VIRTUAL_LIMIT
        && address % PAGE_SIZE == 0
}

unsafe fn table_pointer(entry: u64) -> Result<*mut u64, AddressSpaceError> {
    let physical = entry & ADDRESS_MASK;
    if physical == 0 || physical >= FOUR_GIB {
        return Err(AddressSpaceError::PageTableOutsideIdentityMap);
    }
    Ok(physical as *mut u64)
}

unsafe fn ensure_user_table(entry_pointer: *mut u64) -> Result<*mut u64, AddressSpaceError> {
    let entry = unsafe { ptr::read_volatile(entry_pointer) };
    if entry & PRESENT == 0 {
        let physical = memory::alloc_page_below_4g().ok_or(AddressSpaceError::OutOfMemory)?;
        let table = physical as *mut u64;
        unsafe {
            ptr::write_bytes(table.cast::<u8>(), 0, PAGE_SIZE as usize);
            ptr::write_volatile(entry_pointer, physical | PRESENT | WRITABLE | USER);
        }
        return Ok(table);
    }
    if entry & LARGE_PAGE != 0 {
        return Err(AddressSpaceError::LargePageConflict);
    }
    if entry & USER == 0 {
        unsafe { ptr::write_volatile(entry_pointer, entry | USER) };
    }
    unsafe { table_pointer(entry) }
}

unsafe fn table_is_empty(table: *mut u64) -> bool {
    for index in 0..512usize {
        if unsafe { ptr::read_volatile(table.add(index)) } & PRESENT != 0 {
            return false;
        }
    }
    true
}

/// Capture the production kernel root before any process address space is activated.
pub fn initialize<W: Write>(log: &mut W) -> Result<(), AddressSpaceError> {
    let mut cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    cr3 &= ADDRESS_MASK;
    if cr3 == 0 || cr3 >= FOUR_GIB || cr3 % PAGE_SIZE != 0 {
        return Err(AddressSpaceError::InvalidKernelRoot);
    }
    let root = cr3 as *const u64;
    if unsafe { ptr::read_volatile(root) } & PRESENT == 0 {
        return Err(AddressSpaceError::InvalidKernelRoot);
    }
    KERNEL_ROOT.compare_exchange(0, cr3, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| AddressSpaceError::AlreadyInitialized)?;
    let _ = writeln!(log, "AddressSpace: captured kernel root {:#x}", cr3);
    Ok(())
}

impl AddressSpace {
    /// Create an independent PML4 root, sharing supervisor kernel mappings only.
    pub fn new() -> Result<Self, AddressSpaceError> {
        let kernel_root_physical = KERNEL_ROOT.load(Ordering::Acquire);
        if kernel_root_physical == 0 {
            return Err(AddressSpaceError::KernelRootUnavailable);
        }
        let root_physical = memory::alloc_page_below_4g().ok_or(AddressSpaceError::OutOfMemory)?;
        let identity_pdpt_physical = match memory::alloc_page_below_4g() {
            Some(page) => page,
            None => {
                let _ = memory::free_page(root_physical);
                return Err(AddressSpaceError::OutOfMemory);
            }
        };
        let root = root_physical as *mut u64;
        let identity_pdpt = identity_pdpt_physical as *mut u64;
        let kernel_root = kernel_root_physical as *const u64;

        unsafe {
            ptr::write_bytes(root.cast::<u8>(), 0, PAGE_SIZE as usize);
            ptr::write_bytes(identity_pdpt.cast::<u8>(), 0, PAGE_SIZE as usize);
        }

        // Copy supervisor kernel aliases (kernel image, VMM window and heap).
        for index in 256..512usize {
            let entry = unsafe { ptr::read_volatile(kernel_root.add(index)) };
            unsafe { ptr::write_volatile(root.add(index), entry) };
        }

        // Clone, rather than share, the low identity-map PDPT. Its first four
        // entries remain supervisor-only and preserve access to kernel metadata.
        let identity_entry = unsafe { ptr::read_volatile(kernel_root) };
        if identity_entry & PRESENT == 0 {
            let _ = memory::free_page(identity_pdpt_physical);
            let _ = memory::free_page(root_physical);
            return Err(AddressSpaceError::InvalidKernelRoot);
        }
        let source_pdpt_physical = identity_entry & ADDRESS_MASK;
        if source_pdpt_physical == 0 || source_pdpt_physical >= FOUR_GIB {
            let _ = memory::free_page(identity_pdpt_physical);
            let _ = memory::free_page(root_physical);
            return Err(AddressSpaceError::PageTableOutsideIdentityMap);
        }
        let source_pdpt = source_pdpt_physical as *const u64;
        for index in 0..512usize {
            let entry = unsafe { ptr::read_volatile(source_pdpt.add(index)) };
            unsafe { ptr::write_volatile(identity_pdpt.add(index), entry) };
        }
        unsafe {
            ptr::write_volatile(root, identity_pdpt_physical | PRESENT | WRITABLE | USER);
        }

        Ok(Self { root_physical, identity_pdpt_physical })
    }

    pub fn root_physical(&self) -> u64 {
        self.root_physical
    }

    /// Map an allocated frame into this address space's user range.
    /// W^X is enforced; user mappings start at 512 GiB to avoid the identity map.
    pub fn map_user_page(
        &self,
        virtual_address: u64,
        physical_address: u64,
        writable: bool,
        executable: bool,
    ) -> Result<(), AddressSpaceError> {
        if self.root_physical == 0 {
            return Err(AddressSpaceError::Destroyed);
        }
        if !user_address_is_valid(virtual_address) {
            return Err(AddressSpaceError::InvalidVirtualAddress);
        }
        if writable && executable {
            return Err(AddressSpaceError::InvalidPermissions);
        }
        if physical_address == 0
            || physical_address % PAGE_SIZE != 0
            || physical_address >= MAX_PHYSICAL_ADDRESS
        {
            return Err(AddressSpaceError::InvalidPhysicalAddress);
        }
        if !memory::is_allocated_page(physical_address) {
            return Err(AddressSpaceError::PageNotAllocated);
        }

        unsafe {
            let root = self.root_physical as *mut u64;
            let pml4_index = ((virtual_address >> 39) & 0x1ff) as usize;
            let pdpt_index = ((virtual_address >> 30) & 0x1ff) as usize;
            let pd_index = ((virtual_address >> 21) & 0x1ff) as usize;
            let pt_index = ((virtual_address >> 12) & 0x1ff) as usize;

            let pdpt = ensure_user_table(root.add(pml4_index))?;
            let pd = ensure_user_table(pdpt.add(pdpt_index))?;
            let pt = ensure_user_table(pd.add(pd_index))?;
            let pte = pt.add(pt_index);
            if ptr::read_volatile(pte) & PRESENT != 0 {
                return Err(AddressSpaceError::AlreadyMapped);
            }
            let mut flags = PRESENT | USER;
            if writable { flags |= WRITABLE; }
            if !executable { flags |= NO_EXECUTE; }
            ptr::write_volatile(pte, physical_address | flags);
        }
        Ok(())
    }

    /// Confirm a particular frame is mapped at a user virtual address in this root.
    pub fn is_user_page_mapped(&self, virtual_address: u64, expected_physical: u64) -> bool {
        if self.root_physical == 0 || !user_address_is_valid(virtual_address) {
            return false;
        }
        unsafe {
            let root = self.root_physical as *const u64;
            let pml4e = ptr::read_volatile(root.add(((virtual_address >> 39) & 0x1ff) as usize));
            if pml4e & (PRESENT | USER) != (PRESENT | USER) { return false; }
            let Ok(pdpt) = table_pointer(pml4e) else { return false; };
            let pdpte = ptr::read_volatile(pdpt.add(((virtual_address >> 30) & 0x1ff) as usize));
            if pdpte & (PRESENT | USER) != (PRESENT | USER) || pdpte & LARGE_PAGE != 0 { return false; }
            let Ok(pd) = table_pointer(pdpte) else { return false; };
            let pde = ptr::read_volatile(pd.add(((virtual_address >> 21) & 0x1ff) as usize));
            if pde & (PRESENT | USER) != (PRESENT | USER) || pde & LARGE_PAGE != 0 { return false; }
            let Ok(pt) = table_pointer(pde) else { return false; };
            let pte = ptr::read_volatile(pt.add(((virtual_address >> 12) & 0x1ff) as usize));
            pte & (PRESENT | USER) == (PRESENT | USER)
                && pte & ADDRESS_MASK == expected_physical
        }
    }

    /// Remove a user mapping and reclaim empty tables owned by the PMM.
    pub fn unmap_user_page(
        &self,
        virtual_address: u64,
        expected_physical: u64,
    ) -> Result<(), AddressSpaceError> {
        if self.root_physical == 0 {
            return Err(AddressSpaceError::Destroyed);
        }
        if !user_address_is_valid(virtual_address) {
            return Err(AddressSpaceError::InvalidVirtualAddress);
        }
        if expected_physical == 0
            || expected_physical % PAGE_SIZE != 0
            || expected_physical >= MAX_PHYSICAL_ADDRESS
        {
            return Err(AddressSpaceError::InvalidPhysicalAddress);
        }
        unsafe {
            let root = self.root_physical as *mut u64;
            let pml4_index = ((virtual_address >> 39) & 0x1ff) as usize;
            let pdpt_index = ((virtual_address >> 30) & 0x1ff) as usize;
            let pd_index = ((virtual_address >> 21) & 0x1ff) as usize;
            let pt_index = ((virtual_address >> 12) & 0x1ff) as usize;

            let pml4e = ptr::read_volatile(root.add(pml4_index));
            if pml4e & (PRESENT | USER) != (PRESENT | USER) {
                return Err(AddressSpaceError::NotMapped);
            }
            let pdpt = table_pointer(pml4e)?;
            let pdpte = ptr::read_volatile(pdpt.add(pdpt_index));
            if pdpte & (PRESENT | USER) != (PRESENT | USER) { return Err(AddressSpaceError::NotMapped); }
            if pdpte & LARGE_PAGE != 0 { return Err(AddressSpaceError::LargePageConflict); }
            let pd = table_pointer(pdpte)?;
            let pde = ptr::read_volatile(pd.add(pd_index));
            if pde & (PRESENT | USER) != (PRESENT | USER) { return Err(AddressSpaceError::NotMapped); }
            if pde & LARGE_PAGE != 0 { return Err(AddressSpaceError::LargePageConflict); }
            let pt = table_pointer(pde)?;
            let pte = pt.add(pt_index);
            let leaf = ptr::read_volatile(pte);
            if leaf & (PRESENT | USER) != (PRESENT | USER) { return Err(AddressSpaceError::NotMapped); }
            if leaf & ADDRESS_MASK != expected_physical { return Err(AddressSpaceError::MappingConflict); }

            let pt_physical = pde & ADDRESS_MASK;
            let pd_physical = pdpte & ADDRESS_MASK;
            let pdpt_physical = pml4e & ADDRESS_MASK;
            ptr::write_volatile(pte, 0);
            let mut current_cr3: u64;
            asm!("mov {}, cr3", out(reg) current_cr3, options(nomem, nostack, preserves_flags));
            if current_cr3 & ADDRESS_MASK == self.root_physical {
                asm!("invlpg [{}]", in(reg) virtual_address, options(nostack, preserves_flags));
            }

            if memory::is_allocated_page(pt_physical) && table_is_empty(pt) {
                ptr::write_volatile(pd.add(pd_index), 0);
                if !memory::free_page(pt_physical) { return Err(AddressSpaceError::FreeFailed); }
                if memory::is_allocated_page(pd_physical) && table_is_empty(pd) {
                    ptr::write_volatile(pdpt.add(pdpt_index), 0);
                    if !memory::free_page(pd_physical) { return Err(AddressSpaceError::FreeFailed); }
                    if memory::is_allocated_page(pdpt_physical) && table_is_empty(pdpt) {
                        ptr::write_volatile(root.add(pml4_index), 0);
                        if !memory::free_page(pdpt_physical) { return Err(AddressSpaceError::FreeFailed); }
                    }
                }
            }
        }
        Ok(())
    }

    /// Switch CR3 to this address space. The caller must restore the prior root.
    pub unsafe fn activate(&self) -> Result<u64, AddressSpaceError> {
        if self.root_physical == 0 {
            return Err(AddressSpaceError::Destroyed);
        }
        let mut previous: u64;
        unsafe { asm!("mov {}, cr3", out(reg) previous, options(nomem, nostack, preserves_flags)) };
        unsafe { asm!("mov cr3, {}", in(reg) self.root_physical, options(nostack, preserves_flags)) };
        Ok(previous)
    }

    /// Restore a previously saved CR3 value, including any low control bits.
    pub unsafe fn restore(previous_cr3: u64) -> Result<(), AddressSpaceError> {
        let root = previous_cr3 & ADDRESS_MASK;
        if root == 0 || root >= FOUR_GIB {
            return Err(AddressSpaceError::InvalidKernelRoot);
        }
        unsafe { asm!("mov cr3, {}", in(reg) previous_cr3, options(nostack, preserves_flags)) };
        Ok(())
    }

    /// Free an inactive address-space root after all user mappings have been removed.
    pub fn destroy(&mut self) -> Result<(), AddressSpaceError> {
        if self.root_physical == 0 {
            return Err(AddressSpaceError::Destroyed);
        }
        let mut cr3: u64;
        unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
        if cr3 & ADDRESS_MASK == self.root_physical {
            return Err(AddressSpaceError::ActiveAddressSpace);
        }
        let root = self.root_physical as *mut u64;
        for index in 1..256usize {
            if unsafe { ptr::read_volatile(root.add(index)) } & PRESENT != 0 {
                return Err(AddressSpaceError::AddressSpaceBusy);
            }
        }
        let identity_entry = unsafe { ptr::read_volatile(root) };
        if identity_entry & ADDRESS_MASK != self.identity_pdpt_physical {
            return Err(AddressSpaceError::InvalidKernelRoot);
        }
        if !memory::free_page(self.identity_pdpt_physical) {
            return Err(AddressSpaceError::FreeFailed);
        }
        if !memory::free_page(self.root_physical) {
            self.root_physical = 0;
            self.identity_pdpt_physical = 0;
            return Err(AddressSpaceError::FreeFailed);
        }
        self.root_physical = 0;
        self.identity_pdpt_physical = 0;
        Ok(())
    }
}

/// Validate isolation, user permission bits, CR3 switching, access, and cleanup.
pub fn self_test<W: Write>(log: &mut W) -> Result<(), AddressSpaceError> {
    let free_before = memory::free_page_count();
    let mut first = AddressSpace::new()?;
    let mut second = AddressSpace::new()?;
    let physical = memory::alloc_page_above_4g()
        .or_else(memory::alloc_page)
        .ok_or(AddressSpaceError::OutOfMemory)?;
    first.map_user_page(USER_VIRTUAL_BASE, physical, true, false)?;
    if !first.is_user_page_mapped(USER_VIRTUAL_BASE, physical)
        || second.is_user_page_mapped(USER_VIRTUAL_BASE, physical)
    {
        return Err(AddressSpaceError::VerificationFailed);
    }

    let previous = unsafe { first.activate()? };
    let pointer = USER_VIRTUAL_BASE as *mut u64;
    let expected = 0x4152_4348_5553_4552u64 ^ physical;
    unsafe { ptr::write_volatile(pointer, expected) };
    let actual = unsafe { ptr::read_volatile(pointer) };
    let unmap_result = first.unmap_user_page(USER_VIRTUAL_BASE, physical);
    let restore_result = unsafe { AddressSpace::restore(previous) };
    unmap_result?;
    restore_result?;
    if actual != expected {
        let _ = memory::free_page(physical);
        return Err(AddressSpaceError::VerificationFailed);
    }
    if !memory::free_page(physical) {
        return Err(AddressSpaceError::FreeFailed);
    }
    first.destroy()?;
    second.destroy()?;
    if memory::free_page_count() != free_before {
        return Err(AddressSpaceError::FreeFailed);
    }
    let _ = writeln!(log, "AddressSpace: isolated user mapping, CR3 switch, access, unmap and cleanup passed");
    Ok(())
}
