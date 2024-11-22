use xhci::ring::trb::{transfer::Allowed, Link, BYTES};

use super::TRBRingErr;
use crate::mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, PAGE_SIZE};

/// The Enqueue and Dequeue Pointer s are set to
/// the address of the first TRB location in the Transfer Ring and written to the
/// Endpoint/Stream Context TR Dequeue Pointer field, when a Transfer Ring is
/// initially set up.
///
/// Software uses the Dequeue Pointer to determine when a Transfer Ring is full. As
/// it processes Transfer Events, it updates its copy of the Dequeue Pointer with the
/// value of the Transfer Event TRB Pointer field.
///
/// Transfer Rings support Transfer Descriptors (TDs) that consists of 1 or more
/// TRBs. The TRB Chain (C) bit is set in all but the last TRB of a TD.
///
/// If an error is detected while processing a multi-TRB TD, the xHC shall generate a
/// Transfer Event for the TRB that the error was detected on with the appropriate
/// error Condition Code, then may advance to the next TD. If in the process of
/// advancing to the next TD, a Transfer TRB is encountered with its IOC flag set,
/// then the Condition Code of the Transfer Event generated for that Transfer TRB
/// should be Success, because there was no error actually associated with the TRB
/// that generated the Event.
#[derive(Debug)]
pub struct TransferRing {
    dma: DmaCoherent,
    nr_trbs: usize,
    enqueue_idx: usize,
    dequeue_idx: usize,
    cycle_state: bool,
}

impl TransferRing {
    pub fn with_capacity(nr_trbs: usize) -> Self {
        let nbytes = nr_trbs * BYTES;
        let nframes = nbytes.div_ceil(PAGE_SIZE);
        let segment = FrameAllocOptions::new().alloc_segment(nframes).unwrap();
        let dma = DmaCoherent::map(segment.into(), true).unwrap();

        let mut new_link: Link = Link::new();
        new_link.set_toggle_cycle();
        new_link.set_ring_segment_pointer(dma.start_paddr() as u64);
        let link = {
            let pa = dma.start_paddr() + (nr_trbs - 1) * BYTES;
            let va = paddr_to_vaddr(pa);
            unsafe { &mut *(va as *mut Link) }
        };
        *link = new_link;

        Self {
            dma,
            nr_trbs,
            enqueue_idx: 0,
            dequeue_idx: 0,
            cycle_state: true,
        }
    }

    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    fn is_full(&self) -> bool {
        let next_enqueue_idx = (self.enqueue_idx + 1) % (self.nr_trbs - 1);
        next_enqueue_idx == self.dequeue_idx
    }

    fn link_trb(&mut self) -> &mut Link {
        let pa = self.dma.start_paddr() + (self.nr_trbs - 1) * BYTES;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut Link) }
    }

    fn raw_trb(&mut self, idx: usize) -> &mut [u32; 4] {
        debug_assert!(idx < self.nr_trbs - 1);
        let pa = self.dma.start_paddr() + idx * BYTES;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut [u32; 4]) }
    }

    pub fn enqueue(&mut self, mut trb: Allowed) -> Result<(), TRBRingErr> {
        if self.is_full() {
            return Err(TRBRingErr::Full);
        }

        if self.cycle_state {
            trb.set_cycle_bit();
        } else {
            trb.clear_cycle_bit();
        }
        let raw_trb = self.raw_trb(self.enqueue_idx);
        raw_trb.copy_from_slice(&trb.into_raw());

        if self.enqueue_idx + 1 == self.nr_trbs - 1 {
            if self.cycle_state {
                self.link_trb().set_cycle_bit();
            } else {
                self.link_trb().clear_cycle_bit();
            }

            self.cycle_state = !self.cycle_state;
        }
        self.enqueue_idx = (self.enqueue_idx + 1) % (self.nr_trbs - 1);
        Ok(())
    }

    pub fn set_dequeue_pointer(&mut self, addr: usize) -> Result<(), TRBRingErr> {
        debug_assert!(addr >= self.base() && addr & (BYTES - 1) == 0); // aligned to BYTES
        let idx = (addr - self.base()) / BYTES;
        if idx >= self.nr_trbs - 1 {
            return Err(TRBRingErr::InvalidPtr);
        }
        self.dequeue_idx = idx;
        Ok(())
    }

    pub fn should_skip(&self, addr: usize) -> bool {
        let idx = (addr - self.base()) / BYTES;
        !(self.dequeue_idx < idx && idx < self.enqueue_idx)
            || (self.enqueue_idx <= idx && idx <= self.dequeue_idx)
    }
}
