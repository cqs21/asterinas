use alloc::vec::Vec;

use xhci::ring::trb::{event::Allowed, BYTES};

use super::{TRBRingErr, DEFAULT_RING_SIZE};
use crate::mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, PAGE_SIZE};

/// A fundamental difference between an Event Ring and a Transfer or Command
/// Ring is that the xHC is the producer and system software is the consumer of
/// Event TRBs. The xHC writes Event TRBs to the Event Ring and updates the Cycle
/// bit in the TRBs to indicate to software the current position of the Enqueue
/// Pointer.
///
/// Software maintains an Event Ring Consumer Cycle State (CCS) bit, initializing it
/// to ‘1’ and toggling it every time the Event Ring Dequeue Pointer wraps back to
/// the beginning of the Event Ring.
///
/// If the Event
/// TRB Cycle bit is not equal to CCS, then software stops processing Event TRBs
/// and waits for an interrupt from the xHC for the Event Ring.
///
/// System software shall write the Event Ring Dequeue Pointer (ERDP) register to
/// inform the xHC that it has completed the processing of Event TRBs up to and
/// including the Event TRB referenced by the ERDP.
#[derive(Debug)]
pub struct EventRing {
    dma: DmaCoherent,
    nr_trbs: usize,
    dequeue_idx: usize,
}

impl EventRing {
    pub fn with_capacity(nr_trbs: usize) -> Self {
        let nbytes = nr_trbs * BYTES;
        let nframes = nbytes.div_ceil(PAGE_SIZE);
        let segment = FrameAllocOptions::new().alloc_segment(nframes).unwrap();
        let dma = DmaCoherent::map(segment.into(), true).unwrap();

        Self {
            dma,
            nr_trbs,
            dequeue_idx: 0,
        }
    }

    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    pub fn size(&self) -> usize {
        self.nr_trbs
    }

    fn raw_trb(&mut self, idx: usize) -> &mut [u32; 4] {
        debug_assert!(idx < self.nr_trbs);
        let pa = self.dma.start_paddr() + idx * BYTES;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut [u32; 4]) }
    }

    pub fn can_dequeue(&self) -> bool {
        self.dequeue_idx < self.nr_trbs
    }

    pub fn dequeue(&mut self, cycle_state: bool) -> Result<Allowed, TRBRingErr> {
        let raw_trb = self.raw_trb(self.dequeue_idx);
        let allowed = Allowed::try_from(*raw_trb).map_err(|_| TRBRingErr::UnknownTRB)?;
        if allowed.cycle_bit() != cycle_state {
            return Err(TRBRingErr::Empty);
        }

        self.dequeue_idx += 1;
        Ok(allowed)
    }

    pub fn reset_dequeue_idx(&mut self) {
        self.dequeue_idx = 0;
    }

    pub fn current_dequeue_pointer(&self) -> usize {
        self.dma.start_paddr() + self.dequeue_idx * BYTES
    }
}

/// The Event Ring Segment Table (ERST) is used to define multi-segment Event
/// Rings and to enable runtime expansion and shrinking of the Event Ring.
///
/// +-------------------------------------------------------------------------------------+
//  |              Ring Segment Base Address Lo[31:6]                  |    RsvdZ[5:0]    |
//  +-------------------------------------------------------------------------------------+
//  |                         Ring Segment Base Address Hi[31:0]                          |
//  +------------------------------------------+------------------------------------------+
//  |                RsvdZ[31:16]              |         Ring Segment Size[15:0]          |
//  +------------------------------------------+------------------------------------------+
//  |                                     RsvdZ[31:0]                                     |
//  +-------------------------------------------------------------------------------------+
#[derive(Debug)]
pub struct EventRingSegmentTable {
    dma: DmaCoherent,
    event_rings: Vec<EventRing>,
    current_ring_idx: usize,
    cycle_state: bool,
}

const MAX_EVENT_RING_SEGMENT_TABLE_SIZE: usize = 256; // 4096 / 16

impl EventRingSegmentTable {
    pub fn alloc() -> Self {
        let segments = FrameAllocOptions::new().alloc_segment(1).unwrap();
        let dma = DmaCoherent::map(segments.into(), true).unwrap();

        let default_ring = EventRing::with_capacity(DEFAULT_RING_SIZE);
        let default_segment = unsafe {
            let va = paddr_to_vaddr(dma.start_paddr());
            &mut *(va as *mut [u64; 2])
        };
        default_segment[0] = default_ring.base() as u64;
        default_segment[1] = default_ring.size() as u64;

        let mut event_rings = Vec::new();
        event_rings.push(default_ring);

        Self {
            dma,
            event_rings,
            current_ring_idx: 0,
            cycle_state: true,
        }
    }

    pub fn base(&self) -> usize {
        self.dma.start_paddr()
    }

    pub fn size(&self) -> usize {
        self.event_rings.len()
    }

    pub fn current_dequeue_pointer(&self) -> usize {
        debug_assert!(self.current_ring_idx < self.event_rings.len());
        if self.event_rings[self.current_ring_idx].can_dequeue() {
            return self.event_rings[self.current_ring_idx].current_dequeue_pointer();
        }

        let idx = (self.current_ring_idx + 1) % self.size();
        self.event_rings[idx].base()
    }

    fn raw_segment(&mut self, idx: usize) -> &mut [u64; 2] {
        debug_assert!(idx < MAX_EVENT_RING_SEGMENT_TABLE_SIZE);
        let pa = self.dma.start_paddr() + idx * 16;
        let va = paddr_to_vaddr(pa);
        unsafe { &mut *(va as *mut [u64; 2]) }
    }

    pub fn add_event_ring(&mut self, event_ring: EventRing) {
        let idx = self.event_rings.len();
        let raw_segment = self.raw_segment(idx);
        raw_segment[0] = event_ring.base() as u64;
        raw_segment[1] = event_ring.size() as u64;
        self.event_rings.push(event_ring);
    }

    pub fn dequeue(&mut self) -> Result<Allowed, TRBRingErr> {
        if self.event_rings[self.current_ring_idx].can_dequeue() {
            return self.event_rings[self.current_ring_idx].dequeue(self.cycle_state);
        } else {
            self.event_rings[self.current_ring_idx].reset_dequeue_idx();
            self.current_ring_idx += 1;
        }

        if self.current_ring_idx == self.size() {
            self.cycle_state = !self.cycle_state;
            self.current_ring_idx = 0;
        }

        self.event_rings[self.current_ring_idx].dequeue(self.cycle_state)
    }
}
