// SPDX-License-Identifier: MPL-2.0

//! MSI-X capability support.

#![allow(dead_code)]
#![allow(unused_variables)]

use alloc::{sync::Arc, vec::Vec};

use acpi::platform::interrupt::InterruptModel;
use cfg_if::cfg_if;

use crate::{
    arch::kernel::ACPI_TABLES,
    bus::pci::{
        cfg_space::{Bar, Command, MemoryBar},
        common_device::PciCommonDevice,
        device_info::PciDeviceLocation,
    },
    mm::VmIo,
    trap::IrqLine,
};

cfg_if! {
    if #[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))] {
        use ::tdx_guest::tdx_is_enabled;
        use crate::arch::tdx_guest;
    }
}

/// MSI-X capability. It will set the BAR space it uses to be hidden.
#[derive(Debug)]
#[repr(C)]
pub struct CapabilityMsixData {
    loc: PciDeviceLocation,
    ptr: u16,
    table_size: u16,
    /// MSIX table entry content:
    /// | Vector Control: u32 | Msg Data: u32 | Msg Upper Addr: u32 | Msg Addr: u32 |
    table_bar: Arc<MemoryBar>,
    /// Pending bits table.
    pending_table_bar: Arc<MemoryBar>,
    table_offset: usize,
    pending_table_offset: usize,
    irqs: Vec<Option<IrqLine>>,
}

impl Clone for CapabilityMsixData {
    fn clone(&self) -> Self {
        let new_vec = self.irqs.clone().to_vec();
        Self {
            loc: self.loc,
            ptr: self.ptr,
            table_size: self.table_size,
            table_bar: self.table_bar.clone(),
            pending_table_bar: self.pending_table_bar.clone(),
            irqs: new_vec,
            table_offset: self.table_offset,
            pending_table_offset: self.pending_table_offset,
        }
    }
}

impl CapabilityMsixData {
    pub(super) fn new(dev: &mut PciCommonDevice, cap_ptr: u16) -> Self {
        // Get Table and PBA offset, provide functions to modify them
        let msg_ctrl = dev.location().read16(cap_ptr + 2);
        let table_info = dev.location().read32(cap_ptr + 4);
        let pba_info = dev.location().read32(cap_ptr + 8);
        crate::early_print!(
            "mc:0x{:X},tbi:0x{:X},pba:0x{:X}",
            msg_ctrl,
            table_info,
            pba_info
        );

        let table_bar;
        let pba_bar;

        let bar_manager = dev.bar_manager_mut();
        bar_manager.set_invisible((pba_info & 0b111) as u8);
        bar_manager.set_invisible((table_info & 0b111) as u8);
        match bar_manager
            .bar_space_without_invisible((pba_info & 0b111) as u8)
            .expect("MSIX cfg:pba BAR is none")
        {
            Bar::Memory(memory) => {
                pba_bar = memory;
            }
            Bar::Io(_) => {
                panic!("MSIX cfg:pba BAR is IO type")
            }
        };
        match bar_manager
            .bar_space_without_invisible((table_info & 0b111) as u8)
            .expect("MSIX cfg:table BAR is none")
        {
            Bar::Memory(memory) => {
                table_bar = memory;
            }
            Bar::Io(_) => {
                panic!("MSIX cfg:table BAR is IO type")
            }
        }

        let pba_offset = (pba_info & !(0b111u32)) as usize;
        let table_offset = (table_info & !(0b111u32)) as usize;

        let table_size = (dev.location().read16(cap_ptr + 2) & 0b11_1111_1111) + 1;
        // TODO: Different architecture seems to have different, so we should set different address here.
        let platform_info = ACPI_TABLES.get().unwrap().lock().platform_info().unwrap();
        let (msg_addr_upper, msg_addr_lower) = match platform_info.interrupt_model {
            InterruptModel::Apic(apic) => {
                let upper = (apic.local_apic_address >> 32) as u32;
                let lower = (apic.local_apic_address & 0xFFFF_FFFF) as u32;
                (upper, lower)
            }
            _ => (0x0u32, 0xFEE0_0000u32),
        };

        // Set message address
        for i in 0..table_size {
            #[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))]
            // SAFETY:
            // This is safe because we are ensuring that the physical address of the MSI-X table is valid before this operation.
            // We are also ensuring that we are only unprotecting a single page.
            // The MSI-X table will not exceed one page size, because the size of an MSI-X entry is 16 bytes, and 256 entries are required to fill a page,
            // which is just equal to the number of all the interrupt numbers on the x86 platform.
            // It is better to add a judgment here in case the device deliberately uses so many interrupt numbers.
            // In addition, due to granularity, the minimum value that can be set here is only one page.
            // Therefore, we are not causing any undefined behavior or violating any of the requirements of the `unprotect_gpa_range` function.
            if tdx_is_enabled() {
                unsafe {
                    tdx_guest::unprotect_gpa_range(table_bar.io_mem().paddr(), 1).unwrap();
                }
            }
            // Set message address and disable this msix entry
            table_bar
                .io_mem()
                .write_val((16 * i) as usize + table_offset, &msg_addr_lower)
                .unwrap();
            table_bar
                .io_mem()
                .write_val((16 * i + 4) as usize + table_offset, &msg_addr_upper)
                .unwrap();
            table_bar
                .io_mem()
                .write_val((16 * i + 12) as usize + table_offset, &1_u32)
                .unwrap();
        }

        // enable MSI-X, bit15: MSI-X Enable
        dev.location()
            .write16(cap_ptr + 2, dev.location().read16(cap_ptr + 2) | 0x8000);
        let msg_ctrl = dev.location().read16(cap_ptr + 2);
        crate::early_println!(",msg_ctrl:0x{:X}", msg_ctrl);
        // disable INTx, enable Bus master.
        dev.set_command(dev.command() | Command::INTERRUPT_DISABLE | Command::BUS_MASTER);

        let mut irqs = Vec::with_capacity(table_size as usize);
        for i in 0..table_size {
            irqs.push(None);
        }

        Self {
            loc: *dev.location(),
            ptr: cap_ptr,
            table_size: (dev.location().read16(cap_ptr + 2) & 0b11_1111_1111) + 1,
            table_bar,
            pending_table_bar: pba_bar,
            irqs,
            table_offset,
            pending_table_offset: pba_offset,
        }
    }

    /// MSI-X Table size
    pub fn table_size(&self) -> u16 {
        // bit 10:0 table size
        (self.loc.read16(self.ptr + 2) & 0b11_1111_1111) + 1
    }

    /// Enables an interrupt line, it will replace the old handle with the new handle.
    pub fn set_interrupt_vector(&mut self, handle: IrqLine, index: u16) {
        if index >= self.table_size {
            return;
        }
        self.table_bar
            .io_mem()
            .write_val(
                (16 * index + 8) as usize + self.table_offset,
                &(handle.num() as u32),
            )
            .unwrap();
        let old_handles = core::mem::replace(&mut self.irqs[index as usize], Some(handle));
        // Enable this msix vector
        self.table_bar
            .io_mem()
            .write_val((16 * index + 12) as usize + self.table_offset, &0_u32)
            .unwrap();
    }

    /// Gets mutable IrqLine. User can register callbacks by using this function.
    pub fn irq_mut(&mut self, index: usize) -> Option<&mut IrqLine> {
        self.irqs[index].as_mut()
    }

    /// Returns true if MSI-X Enable bit is set.
    pub fn is_enabled(&self) -> bool {
        let msg_ctrl = self.loc.read16(self.ptr + 2);
        msg_ctrl & 0x8000 != 0
    }
}

fn set_bit(origin_value: u16, offset: usize, set: bool) -> u16 {
    (origin_value & (!(1 << offset))) | ((set as u16) << offset)
}

/*
=====
00:00.0 Host bridge: Intel Corporation 440FX - 82441FX PMC [Natoma] (rev 02)

bus 00, slot 00, func 0, vend:dev:s_vend:s_dev:rev 8086:1237:1af4:1100:02
class 06, sub_class 00 prog_if 00, hdr 0, flags <>, irq 0
  00: 86 80 37 12 07 00 00 00 02 00 00 06 00 00 00 00  "..7............."
  10: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "................"
  30: 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 00 00  "................"

=====
00:01.0 ISA bridge: Intel Corporation 82371SB PIIX3 ISA [Natoma/Triton II]
bus 00, slot 01, func 0, vend:dev:s_vend:s_dev:rev 8086:7000:1af4:1100:00
class 06, sub_class 01 prog_if 00, hdr 0, flags <>, irq 0
  00: 86 80 00 70 07 00 00 02 00 00 01 06 00 00 80 00  "...p............"
  10: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "................"
  30: 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 00 00  "................"

====
00:01.1 IDE interface: Intel Corporation 82371SB PIIX3 IDE [Natoma/Triton II]
Region 0: Memory at 000001f0 (32-bit, non-prefetchable) [virtual] [size=8]
Region 1: Memory at 000003f0 (type 3, non-prefetchable) [virtual]
Region 2: Memory at 00000170 (32-bit, non-prefetchable) [virtual] [size=8]
Region 3: Memory at 00000370 (type 3, non-prefetchable) [virtual]
Region 4: I/O ports at c060 [virtual] [size=16]

bus 00, slot 01, func 1, vend:dev:s_vend:s_dev:rev 8086:7010:1af4:1100:00
class 01, sub_class 01 prog_if 80, hdr 0, flags <>, irq 0
  addr0 000001f0, size 00000008
  addr1 000003f6, size 00000001
  addr2 00000170, size 00000008
  addr3 00000376, size 00000001
  addr4 0000c060, size 00000010
  00: 86 80 10 70 07 00 80 02 00 80 01 01 00 00 00 00  "...p............"
  10: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 61 c0 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "a..............."
  30: 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 00 00  "................"

====
00:01.2 USB controller: Intel Corporation 82371SB PIIX3 USB [Natoma/Triton II] (rev 01)
Interrupt: pin D routed to IRQ 10
Region 4: I/O ports at c040 [size=32]

bus 00, slot 01, func 2, vend:dev:s_vend:s_dev:rev 8086:7020:1af4:1100:01
class 0c, sub_class 03 prog_if 00, hdr 0, flags <>, irq 10
  addr4 0000c040, size 00000020
  00: 86 80 20 70 03 00 00 00 01 00 03 0c 00 00 00 00  ".. p............"
  10: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 41 c0 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "A..............."
  30: 00 00 00 00 00 00 00 00 00 00 00 00 0b 04 00 00  "................"

====
00:01.3 Bridge: Intel Corporation 82371AB/EB/MB PIIX4 ACPI (rev 03)
Interrupt: pin A routed to IRQ 9

bus 00, slot 01, func 3, vend:dev:s_vend:s_dev:rev 8086:7113:1af4:1100:03
class 06, sub_class 80 prog_if 00, hdr 0, flags <>, irq 9
  00: 86 80 13 71 07 00 80 02 03 00 80 06 00 00 00 00  "...q............"
  10: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "................"
  30: 00 00 00 00 00 00 00 00 00 00 00 00 0a 01 00 00  "................"

====
00:02.0 VGA compatible controller: Cirrus Logic GD 5446
Region 0: Memory at c0000000 (32-bit, prefetchable) [size=32M]
Region 1: Memory at c2001000 (32-bit, non-prefetchable) [size=4K]
Expansion ROM at 000c0000 [disabled] [size=128K]

bus 00, slot 02, func 0, vend:dev:s_vend:s_dev:rev 1013:00b8:1af4:1100:00
class 03, sub_class 00 prog_if 00, hdr 0, flags <>, irq 0
  addr0 c0000000, size 02000000
  addr1 c2001000, size 00001000
  00: 13 10 b8 00 07 00 00 00 00 00 00 03 00 00 00 00  "................"
  10: 08 00 00 c0 00 10 00 c2 00 00 00 00 00 00 00 00  "................"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 00 11  "................"
  30: 00 00 ff ff 00 00 00 00 00 00 00 00 ff 00 00 00  "................"

====
00:03.0 Communication controller: Red Hat, Inc. Virtio console
Interrupt: pin A routed to IRQ 11
Region 0: I/O ports at c020 [size=32]
Region 1: Memory at c2000000 (32-bit, non-prefetchable) [size=4K]
Capabilities: [40] MSI-X: Enable+ Count=2 Masked-
        Vector table: BAR=1 offset=00000000
        PBA: BAR=1 offset=00000800

bus 00, slot 03, func 0, vend:dev:s_vend:s_dev:rev 1af4:1003:1af4:0003:00
class 07, sub_class 80 prog_if 00, hdr 0, flags <>, irq 11
  addr0 0000c020, size 00000020
  addr1 c2000000, size 00001000
  00: f4 1a 03 10 07 04 10 00 00 00 80 07 00 00 00 00  "................"
  10: 21 c0 00 00 00 00 00 c2 00 00 00 00 00 00 00 00  "!..............."
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 03 00  "................"
  30: 00 00 00 00 40 00 00 00 00 00 00 00 0b 01 00 00  "....@..........."
  40: 11 00  ".."

====
00:04.0 SCSI storage controller: Red Hat, Inc. Virtio block device
Region 0: Memory at c2002000 (64-bit, prefetchable) [size=4K]
Region 2: Memory at c2003000 (64-bit, prefetchable) [size=4K]
Capabilities: [40] MSI-X: Enable+ Count=2 Masked-
        Vector table: BAR=2 offset=00000000
        PBA: BAR=2 offset=00000c00

bus 00, slot 04, func 0, vend:dev:s_vend:s_dev:rev 1af4:1001:1af4:0002:00
class 01, sub_class 00 prog_if 00, hdr 0, flags <>, irq 0
  addr0 c2002000, size 00001000
  addr2 c2003000, size 00001000
  00: f4 1a 01 10 07 04 10 00 00 00 00 01 00 00 00 00  "................"
  10: 0c 20 00 c2 00 00 00 00 0c 30 00 c2 00 00 00 00  ". .......0......"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 02 00  "................"
  30: 00 00 00 00 40 00 00 00 00 00 00 00 ff 00 00 00  "....@..........."
  40: 11 00  ".."

====
00:05.0 Ethernet controller: Red Hat, Inc. Virtio network device
Region 0: Memory at c2004000 (64-bit, prefetchable) [size=4K]
Region 2: Memory at c2005000 (64-bit, prefetchable) [size=4K]
Capabilities: [40] MSI-X: Enable+ Count=4 Masked-
        Vector table: BAR=2 offset=00000000
        PBA: BAR=2 offset=00000c00

bus 00, slot 05, func 0, vend:dev:s_vend:s_dev:rev 1af4:1000:1af4:0001:00
class 02, sub_class 00 prog_if 00, hdr 0, flags <>, irq 0
  addr0 c2004000, size 00001000
  addr2 c2005000, size 00001000
  00: f4 1a 00 10 07 04 10 00 00 00 00 02 00 00 00 00  "................"
  10: 0c 40 00 c2 00 00 00 00 0c 50 00 c2 00 00 00 00  ".@.......P......"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 01 00  "................"
  30: 00 00 00 00 40 00 00 00 00 00 00 00 ff 00 00 00  "....@..........."
  40: 11 00  ".."

====
00:06.0 Unclassified device [00ff]: Red Hat, Inc. Virtio memory balloon
Interrupt: pin A routed to IRQ 11
Region 0: I/O ports at c000 [size=32]

bus 00, slot 06, func 0, vend:dev:s_vend:s_dev:rev 1af4:1002:1af4:0005:00
class 00, sub_class ff prog_if 00, hdr 0, flags <>, irq 11
  addr0 0000c000, size 00000020
  00: f4 1a 02 10 07 00 00 00 00 00 ff 00 00 00 00 00  "................"
  10: 01 c0 00 00 00 00 00 00 00 00 00 00 00 00 00 00  "................"
  20: 00 00 00 00 00 00 00 00 00 00 00 00 f4 1a 05 00  "................"
  30: 00 00 00 00 00 00 00 00 00 00 00 00 0a 01 00 00  "................"

*/
