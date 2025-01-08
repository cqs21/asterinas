use xhci::context::{
    Device32Byte, Device64Byte, DeviceHandler, EndpointHandler, Input32Byte, Input64Byte,
    InputControl, InputHandler, SlotHandler,
};

use super::XhciSlot;
use crate::mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, PAGE_SIZE};

// The Device Context Base Address Array shall contain MaxSlotsEn + 1 entries.
// The maximum size of the Device Context Base Address Array is 256 64 -bit
// entries, or 2K Bytes.
//
// If the Max Scratchpad Buffers field of the HCSPARAMS2 register is > ‘0’, then the
// first entry (entry_0) in the DCBAA shall contain a pointer to the Scratchpad
// Buffer Array. If the Max Scratchpad Buffers field of the HCSPARAMS2 register is
// = ‘0’, then the first entry (entry_0) in the DCBAA is reserved and shall be cleared
// to ‘0’ by software.
//
// If the Context Size (CSZ) field in the HCCPARAMS1 register = '1' then the Device
// Context data structures consume 64 bytes each.
#[derive(Debug)]
pub struct DeviceContexts {
    dma: DmaCoherent,
    max_slots: u8,
    has_scratchpad_buffers: bool,
    is_64bytes_context: bool,
}

impl DeviceContexts {
    pub fn new(max_slots: u8, has_scratchpad_buffers: bool, is_64bytes_context: bool) -> Self {
        debug_assert!(max_slots <= u8::MAX);
        let seg = FrameAllocOptions::new().alloc_segment(1).unwrap();
        let dma = DmaCoherent::map(seg.into(), true).unwrap();
        Self {
            dma,
            max_slots,
            has_scratchpad_buffers,
            is_64bytes_context,
        }
    }

    /// Return the Device Context Base Address Array Pointer (DCBAAP), a 64-bit physical
    /// address pointing to where the Device Context Base Address Array is located.
    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    /// Return the number of Device Context.
    pub fn size(&self) -> usize {
        self.max_slots as usize
    }

    pub fn is_64bytes_context(&self) -> bool {
        self.is_64bytes_context
    }

    fn slot(&self, id: u8) -> usize {
        debug_assert!(id <= self.max_slots);
        self.dma
            .reader()
            .skip((id as usize) * 8)
            .read_once::<usize>()
            .unwrap()
    }

    pub fn set_slot(&mut self, slot: &XhciSlot) {
        debug_assert!(slot.slot_id() <= self.max_slots);
        self.dma
            .writer()
            .skip((slot.slot_id() as usize) * 8)
            .write_once::<usize>(&slot.output_device_context_base())
            .unwrap();
    }

    pub fn clear_slot(&mut self, slot: &XhciSlot) {
        debug_assert!(slot.slot_id() <= self.max_slots);
        self.dma
            .writer()
            .skip((slot.slot_id() as usize) * 8)
            .write_once::<usize>(&0)
            .unwrap();
    }

    fn ctx32(&self, id: u8) -> &mut Device32Byte {
        debug_assert!(id > 0);
        let va = paddr_to_vaddr(self.slot(id));
        unsafe { &mut *(va as *mut Device32Byte) }
    }

    fn ctx64(&self, id: u8) -> &mut Device64Byte {
        debug_assert!(id > 0);
        let va = paddr_to_vaddr(self.slot(id));
        unsafe { &mut *(va as *mut Device64Byte) }
    }

    pub fn device(&self, slot_id: u8) -> &dyn DeviceHandler {
        if self.is_64bytes_context {
            self.ctx64(slot_id)
        } else {
            self.ctx32(slot_id)
        }
    }

    pub fn device_mut(&mut self, slot_id: u8) -> &mut dyn DeviceHandler {
        if self.is_64bytes_context {
            self.ctx64(slot_id)
        } else {
            self.ctx32(slot_id)
        }
    }
}

/// The Input Context data structure specifies the endpoints and the operations to
/// be performed on those endpoints by the Address Device, Configure Endpoint,
/// and Evaluate Context Commands.
///
/// The first entry (offset 000h) of the Input Context shall be the Input Control
/// Context data structure. The remaining entries shall be organized identical ly to
/// the Device Context data structures.
#[derive(Debug)]
pub struct InputContext {
    dma: DmaCoherent,
    is_64bytes_context: bool,
}

impl InputContext {
    pub fn new(is_64bytes_context: bool) -> Self {
        // One page for the input context, the other for the output device context.
        let seg = FrameAllocOptions::new().alloc_segment(2).unwrap();
        let dma = DmaCoherent::map(seg.into(), true).unwrap();
        Self {
            dma,
            is_64bytes_context,
        }
    }

    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    pub fn output_device_context_base(&self) -> usize {
        self.base() + PAGE_SIZE
    }

    fn input_ctx32(&self) -> &mut Input32Byte {
        let va = paddr_to_vaddr(self.base());
        unsafe { &mut *(va as *mut Input32Byte) }
    }

    fn input_ctx64(&self) -> &mut Input64Byte {
        let va = paddr_to_vaddr(self.base());
        unsafe { &mut *(va as *mut Input64Byte) }
    }

    fn output_ctx32(&self) -> &mut Device32Byte {
        let va = paddr_to_vaddr(self.output_device_context_base());
        unsafe { &mut *(va as *mut Device32Byte) }
    }

    fn output_ctx64(&self) -> &mut Device64Byte {
        let va = paddr_to_vaddr(self.base());
        unsafe { &mut *(va as *mut Device64Byte) }
    }

    pub fn handle(&self) -> &dyn InputHandler {
        if self.is_64bytes_context {
            self.input_ctx64()
        } else {
            self.input_ctx32()
        }
    }

    pub fn handle_mut(&mut self) -> &mut dyn InputHandler {
        if self.is_64bytes_context {
            self.input_ctx64()
        } else {
            self.input_ctx32()
        }
    }

    pub fn output_device(&self) -> &dyn DeviceHandler {
        if self.is_64bytes_context {
            self.output_ctx64()
        } else {
            self.output_ctx32()
        }
    }

    pub fn output_device_mut(&mut self) -> &mut dyn DeviceHandler {
        if self.is_64bytes_context {
            self.output_ctx64()
        } else {
            self.output_ctx32()
        }
    }
}
