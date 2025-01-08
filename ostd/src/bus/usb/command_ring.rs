use xhci::ring::trb::{command::Allowed, Link, BYTES};

use crate::mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, PAGE_SIZE};

/// All TRB data structures shall be 16 bytes in size.
/// TRB Rings may be larger than a Page, however they shall not cross a 64K byte
/// boundary.
///
/// All TRBs in the ring shall be cleared to ‘0’. This state represents an empty queue.
/// The Cycle bit shall be initialized by software to ‘0’ in all TRBs of all segments when
/// initializing a ring.
/// The Producer Cycle State (PCS) and the Consumer Cycle State (CCS) bits shall be set
/// to ‘1’ when a ring is initialized.
#[derive(Debug)]
pub struct CommandRing {
    dma: DmaCoherent,
    nr_trbs: usize,
    enqueue_idx: usize,
    dequeue_idx: usize,
    cycle_state: bool,
}

pub const DEFAULT_RING_SIZE: usize = 256;

impl CommandRing {
    pub fn with_capacity(nr_trbs: usize) -> Self {
        let nbytes = nr_trbs * BYTES;
        let nframes = nbytes.div_ceil(PAGE_SIZE);
        let segment = FrameAllocOptions::new().alloc_segment(nframes).unwrap();
        let dma = DmaCoherent::map(segment.into(), true).unwrap();

        let mut new_link = Link::default();
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

    pub fn is_empty(&self) -> bool {
        self.enqueue_idx == self.dequeue_idx
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
}

#[derive(Debug)]
pub enum TRBRingErr {
    Full,
    Empty,
    UnknownTRB,
    InvalidPtr,
}
