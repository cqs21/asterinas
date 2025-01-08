use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
    enqueue_idx: AtomicUsize,
    dequeue_idx: AtomicUsize,
    cycle_state: AtomicBool,
}

pub const DEFAULT_RING_SIZE: usize = 256;

impl CommandRing {
    pub fn with_capacity(nr_trbs: usize) -> Self {
        let nbytes = nr_trbs * BYTES;
        let nframes = nbytes.div_ceil(PAGE_SIZE);
        let segment = FrameAllocOptions::new().alloc_segment(nframes).unwrap();
        let dma = DmaCoherent::map(segment.into(), true).unwrap();

        let mut new_link = Link::new();
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
            enqueue_idx: AtomicUsize::new(0),
            dequeue_idx: AtomicUsize::new(0),
            cycle_state: AtomicBool::new(true),
        }
    }

    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    fn enqueue_idx(&self) -> usize {
        self.enqueue_idx.load(Ordering::Relaxed)
    }

    fn set_enqueue_idx(&self, idx: usize) {
        self.enqueue_idx.store(idx, Ordering::Relaxed);
    }

    fn dequeue_idx(&self) -> usize {
        self.dequeue_idx.load(Ordering::Relaxed)
    }

    fn set_dequeue_idx(&self, idx: usize) {
        self.dequeue_idx.store(idx, Ordering::Relaxed);
    }

    fn cycle_state(&self) -> bool {
        self.cycle_state.load(Ordering::Relaxed)
    }

    fn set_cycle_state(&self, state: bool) {
        self.cycle_state.store(state, Ordering::Relaxed);
    }

    pub fn is_empty(&self) -> bool {
        self.enqueue_idx() == self.dequeue_idx()
    }

    fn is_full(&self) -> bool {
        let next_enqueue_idx = (self.enqueue_idx() + 1) % (self.nr_trbs - 1);
        next_enqueue_idx == self.dequeue_idx()
    }

    fn link_trb(&self) -> &mut Link {
        let pa = self.dma.start_paddr() + (self.nr_trbs - 1) * BYTES;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut Link) }
    }

    fn raw_trb(&self, idx: usize) -> &mut [u32; 4] {
        debug_assert!(idx < self.nr_trbs - 1);
        let pa = self.dma.start_paddr() + idx * BYTES;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut [u32; 4]) }
    }

    pub fn enqueue(&self, mut trb: Allowed) -> Result<(), TRBRingErr> {
        if self.is_full() {
            return Err(TRBRingErr::Full);
        }

        let cycle_state = self.cycle_state();
        let enqueue_idx = self.enqueue_idx();
        if cycle_state {
            trb.set_cycle_bit();
        } else {
            trb.clear_cycle_bit();
        }
        let raw_trb = self.raw_trb(enqueue_idx);
        raw_trb.copy_from_slice(&trb.into_raw());

        if enqueue_idx + 1 == self.nr_trbs - 1 {
            if cycle_state {
                self.link_trb().set_cycle_bit();
            } else {
                self.link_trb().clear_cycle_bit();
            }

            self.set_cycle_state(!cycle_state);
        }
        self.set_enqueue_idx((enqueue_idx + 1) % (self.nr_trbs - 1));
        Ok(())
    }

    pub fn set_dequeue_pointer(&self, addr: usize) -> Result<(), TRBRingErr> {
        debug_assert!(addr >= self.base() && addr & (BYTES - 1) == 0); // aligned to BYTES
        let idx = (addr - self.base()) / BYTES;
        if idx >= self.nr_trbs - 1 {
            return Err(TRBRingErr::InvalidPtr);
        }
        self.set_dequeue_idx(idx);
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
