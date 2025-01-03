use alloc::{borrow::ToOwned, boxed::Box, collections::vec_deque::VecDeque, sync::Arc, vec::Vec};
use core::{
    fmt::Debug,
    hint::spin_loop,
    sync::atomic::{AtomicU16, Ordering::Relaxed},
};

use log::debug;
use num::Zero;
use xhci::{
    context::{EndpointState, EndpointType, SlotState},
    registers::operational::{UsbCommandRegister, UsbStatusRegister},
    ring::trb::{
        command,
        event::{self, CommandCompletion, TransferEvent},
        transfer::{self, TransferType},
    },
    Registers,
};

use super::{
    class::UsbClass, device_contexts::InputContext, transfer_ring::TransferRing, CommandRing,
    DeviceContexts, EventRingSegmentTable, TRBRingErr, XhciSlot, DEFAULT_RING_SIZE, XHCI_DRIVER,
};
use crate::{
    bus::{
        pci::{
            bus::PciDevice,
            capability::{msix::CapabilityMsixData, CapabilityData},
            cfg_space::Bar,
            common_device::PciCommonDevice,
            PciDeviceId,
        },
        usb::xhci_device,
        BusProbeError,
    },
    io_mem::IoMem,
    mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions},
    sync::SpinLock,
    trap::{IrqLine, TrapFrame},
};

#[derive(Debug)]
pub struct XHostControllerId {
    device_id: PciDeviceId,
}

impl PciDevice for XHostControllerId {
    fn device_id(&self) -> PciDeviceId {
        self.device_id
    }
}

#[derive(Debug)]
pub struct XHostController {
    regs: Arc<SpinLock<Registers<IoMem>>>,
    dev_ctxs: DeviceContexts,
    cmd_ring: Arc<CommandRing>,
    seg_tables: Vec<Option<EventRingSegmentTable>>,
    msix: CapabilityMsixData,
    slots: Vec<Option<Arc<XhciSlot>>>,
}

// System software utilizes the Doorbell Register to notify the xHC that it has Device Slot
// related work for the xHC to perform.
pub type DoorbellReason = (u8, u16); // (target, stream_id)

impl XHostController {
    pub(super) fn init(
        pci_device: PciCommonDevice,
    ) -> Result<Arc<dyn PciDevice>, (BusProbeError, PciCommonDevice)> {
        let device_id = pci_device.device_id().clone();

        let io_mem = match pci_device
            .bar_manager()
            .bar_space_without_invisible(0)
            .unwrap()
        {
            Bar::Memory(mem_bar) => mem_bar.io_mem().clone(),
            Bar::Io(_) => {
                return Err((BusProbeError::ConfigurationSpaceError, pci_device));
            }
        };
        let mmio_base = io_mem.paddr();
        let mut regs = unsafe { Registers::new(mmio_base, io_mem) };

        // Reset xHC device.
        regs.operational.usbcmd.update_volatile(|cmd| {
            cmd.set_host_controller_reset();
        });

        // Wait until the Controller Not Ready (CNR) flag in the USBSTS is ‘0’
        // before writing any xHC Operational or Runtime registers.
        while regs
            .operational
            .usbsts
            .read_volatile()
            .controller_not_ready()
        {
            spin_loop();
        }

        // This specifies the maximum number of Device Context Structures and
        // Doorbell Array entries this host controller can support.
        let max_slots = regs
            .capability
            .hcsparams1
            .read_volatile()
            .number_of_device_slots();
        regs.operational.config.update_volatile(|config| {
            // Program the Max Device Slots Enabled (MaxSlotsEn):
            config.set_max_device_slots_enabled(max_slots);
        });

        let has_scratchpad_buffers = !regs
            .capability
            .hcsparams2
            .read_volatile()
            .max_scratchpad_buffers()
            .is_zero();
        let is_64bytes_context = regs.capability.hccparams1.read_volatile().context_size();
        let dev_ctxs = DeviceContexts::new(max_slots, has_scratchpad_buffers, is_64bytes_context);
        regs.operational.dcbaap.update_volatile(|dcbaap| {
            // Program the Device Context Base Address Array Pointer (DCBAAP):
            dcbaap.set(dev_ctxs.base() as u64);
        });

        let mut slots = Vec::with_capacity(max_slots as usize);
        (0..max_slots).for_each(|_| slots.push(None));

        let cmd_ring = CommandRing::with_capacity(DEFAULT_RING_SIZE);
        regs.operational.crcr.update_volatile(|crcr| {
            // The Consumer Cycle State (CCS) bit shall be set to ‘1’ when it is initialized.
            crcr.set_ring_cycle_state();
            // Program the Command Ring Control Register (CRCR):
            crcr.set_command_ring_pointer(cmd_ring.base() as u64);
        });

        // Initialize interrupts.
        // This specifies the number of Interrupters implemented on this host controller.
        let max_intrs = regs
            .capability
            .hcsparams1
            .read_volatile()
            .number_of_interrupts();
        // For MSI-X, table vector entry 0 shall be initialized and enabled, at a minimum.
        // TODO: Support interrupt without MSI-X.
        let msix = pci_device
            .capabilities()
            .iter()
            .find_map(|cap| match cap.capability_data() {
                CapabilityData::Msix(data) => Some(data.clone()),
                _ => None,
            })
            .unwrap();
        debug_assert!(msix.table_size() == max_intrs);
        let mut seg_tables = Vec::with_capacity(max_intrs as usize);
        (0..max_intrs).for_each(|_| seg_tables.push(None));

        let mut xhci_device = Self {
            regs: Arc::new(SpinLock::new(regs)),
            dev_ctxs,
            cmd_ring: Arc::new(cmd_ring),
            seg_tables,
            msix,
            slots,
        };

        // Set the primary interrupter callback.
        xhci_device.set_interrupter(0, default_primary_irq_callback);

        // Turn the host controller ON via setting the Run/Stop (R/S) bit to ‘1’.
        // This operation allows the xHC to begin accepting doorbell references.
        xhci_device.run();

        // Detect attached ports.
        xhci_device.enumerate_ports();

        debug!("{:?}", xhci_device.status());

        XHCI_DRIVER
            .get()
            .unwrap()
            .controllers
            .lock()
            .push(xhci_device);

        Ok(Arc::new(XHostControllerId { device_id }))
    }

    fn run(&mut self) {
        self.regs.lock().operational.usbcmd.update_volatile(|cmd| {
            if cmd.run_stop() == false {
                cmd.set_run_stop();
            }
        });
    }

    fn stop(&mut self) {
        self.regs.lock().operational.usbcmd.update_volatile(|cmd| {
            if cmd.run_stop() == true {
                cmd.clear_run_stop();
            }
        });
    }

    fn status(&self) -> UsbStatusRegister {
        self.regs.lock().operational.usbsts.read_volatile()
    }

    fn set_interrupter<F>(&mut self, idx: usize, callback: F)
    where
        F: Fn(&TrapFrame) + Send + Sync + 'static,
    {
        debug_assert!(idx < self.seg_tables.len());
        if self.seg_tables[idx].is_some() {
            let irq = self.msix.irq_mut(idx).unwrap();
            irq.on_active(callback);
            return;
        }

        // Alloc Event Ring Segment Table.
        let seg_table = EventRingSegmentTable::alloc();
        let mut regs = self.regs.lock();
        // Program the Interrupter Event Ring Segment Table Size (ERSTSZ) register.
        regs.interrupter_register_set
            .interrupter_mut(idx)
            .erstsz
            .update_volatile(|erstsz| {
                erstsz.set(seg_table.size() as u16);
            });
        // Program the Interrupter Event Ring Dequeue Pointer (ERDP) register.
        regs.interrupter_register_set
            .interrupter_mut(idx)
            .erdp
            .update_volatile(|erdp| {
                erdp.set_event_ring_dequeue_pointer(seg_table.current_dequeue_pointer() as u64);
            });
        // Program the Interrupter Event Ring Segment Table Base Address (ERSTBA) register.
        regs.interrupter_register_set
            .interrupter_mut(idx)
            .erstba
            .update_volatile(|erstba| {
                erstba.set(seg_table.base() as u64);
            });
        // The IMODI field shall default to 4000 (1 ms) upon initialization and reset.
        regs.interrupter_register_set
            .interrupter_mut(idx)
            .imod
            .update_volatile(|imod| {
                imod.set_interrupt_moderation_interval(4000);
            });
        let old = self.seg_tables[idx].replace(seg_table);
        debug_assert!(old.is_none());

        // Alloc and enable the MSI-X interrupt entry.
        let mut irq = IrqLine::alloc().unwrap();
        irq.on_active(callback);
        self.msix.set_interrupt_vector(irq, idx as u16);

        // Enable system bus interrupt generation by writing a ‘1’ to the Interrupter Enable (INTE)
        // flag of the USBCMD register.
        regs.operational.usbcmd.update_volatile(|cmd| {
            cmd.set_interrupter_enable();
        });

        // Enable the Interrupter by writing a ‘1’ to the Interrupt Enable (IE) field of
        // the Interrupter Management register.
        regs.interrupter_register_set
            .interrupter_mut(idx)
            .iman
            .update_volatile(|iman| {
                iman.set_interrupt_enable();
            });
    }

    fn irq_num(&mut self, idx: usize) -> Option<usize> {
        debug_assert!(idx < self.seg_tables.len());
        self.msix.irq_mut(idx).map(|irq| irq.num() as usize)
    }

    fn handle_primary_irq(&mut self) {
        // Clear the op reg interrupt status first, so we can receive interrupts from other MSI-X interrupters.
        self.regs.lock().operational.usbsts.update_volatile(|sts| {
            sts.clear_event_interrupt();
        });
        // Clear the primary interrupter pending status.
        self.regs
            .lock()
            .interrupter_register_set
            .interrupter_mut(0)
            .iman
            .update_volatile(|iman| {
                iman.clear_interrupt_pending();
            });

        let event_dequeue_ptr = self.handle_event_ring();

        // crate::early_println!("======updating event dequeue ptr:0x{:x}", event_dequeue_ptr);
        // Update Event Ring Dequeue Pointer.
        self.regs
            .lock()
            .interrupter_register_set
            .interrupter_mut(0)
            .erdp
            .update_volatile(|erdp| {
                erdp.set_event_ring_dequeue_pointer(event_dequeue_ptr as u64);
                // Clear the event handler busy flag.
                erdp.clear_event_handler_busy();
            });
    }

    fn handle_event_ring(&mut self) -> usize {
        while let Ok(allowed) = self.seg_tables[0].as_mut().unwrap().dequeue() {
            debug!("{:?}", allowed);
            // crate::early_println!("{:?}", allowed);
            // TODO: handle error completion_code() for each Event.
            match allowed {
                event::Allowed::CommandCompletion(cc) => self.handle_command_completion(cc),
                event::Allowed::PortStatusChange(psc) => self.handle_port_status_change(psc),
                event::Allowed::TransferEvent(te) => self.handle_transfer_event(te),
                // TODO: handle more Event.
                _ => continue,
            }
        }

        self.seg_tables[0]
            .as_mut()
            .unwrap()
            .current_dequeue_pointer()
    }

    fn handle_command_completion(&mut self, cc: CommandCompletion) {
        let slot_id = cc.slot_id();
        // TODO: how to deal with slot 0, which is utilized by the xHCI Scratchpad mechanism.
        if slot_id != 0 && self.slots[(slot_id - 1) as usize].is_some() {
            self.handle_command_completion_at(slot_id, cc);
        }

        // Update Command Ring Dequeue Pointer.
        self.cmd_ring
            .set_dequeue_pointer(cc.command_trb_pointer() as usize)
            .unwrap();
    }

    fn handle_command_completion_at(&mut self, slot_id: u8, cc: CommandCompletion) {
        let slot = self.slots[(slot_id - 1) as usize].as_mut().unwrap();
        if slot.slot_id() == 0 {
            slot.set_slot_id(slot_id);
            // Load the appropriate (Device Slot ID) entry in the Device Context Base
            // Address Array with a pointer to the Output Device Context.
            self.dev_ctxs.set_slot(slot);
        }

        slot.handle_command_completion(cc);
    }

    fn handle_port_status_change(&mut self, psc: event::PortStatusChange) {
        let port_id = psc.port_id();
        let port = self
            .regs
            .lock()
            .port_register_set
            .read_volatile_at(port_id as usize - 1);
        let prc = port.portsc.port_reset_change();
        let ccs = port.portsc.current_connect_status();
        let csc = port.portsc.connect_status_change();
        let ped = port.portsc.port_enabled_disabled();
        let port_speed = port.portsc.port_speed();
        if prc == true && ccs == true && csc == true && ped == true {
            self.add_port(port_id, port_speed);
        } else {
            self.remove_port(port_id);
        }
    }

    fn handle_transfer_event(&mut self, te: TransferEvent) {
        let slot_id = te.slot_id();
        let slot = self.slots[(slot_id - 1) as usize].as_mut().unwrap();
        slot.handle_transfer_event(te);
    }

    fn add_command(&mut self, command: command::Allowed) -> Result<(), TRBRingErr> {
        self.cmd_ring.enqueue(command)
    }

    fn ring_doorbell_at(&mut self, idx: usize, target: u8, stream_id: u16) {
        debug_assert!(idx <= self.dev_ctxs.size());
        self.regs.lock().doorbell.update_volatile_at(idx, |db| {
            db.set_doorbell_target(target);
            db.set_doorbell_stream_id(stream_id);
        });
    }

    fn enumerate_ports(&mut self) {
        let max_port = self.regs.lock().port_register_set.len();
        for idx in 0..max_port {
            let mut port = self.regs.lock().port_register_set.read_volatile_at(idx);
            // When the xHC detects a device attach, it shall set the Current Connect
            // Status (CCS) and Connect Status Change (CSC) flags to ‘1’. If the
            // assertion of CSC results in a ‘0’ to ‘1’ transition of Port Status Change
            // Event Generation (PSCEG, section 4.19.2), the xHC shall generate a Port
            // Status Change Event.
            let ccs = port.portsc.current_connect_status();
            let csc = port.portsc.connect_status_change();
            if ccs == false || csc == false {
                continue;
            }

            let ped = port.portsc.port_enabled_disabled();
            let pr = port.portsc.port_reset();
            let pls = port.portsc.port_link_state();
            let port_speed = port.portsc.port_speed();
            // A USB3 protocol port attempts to automatically advance to the
            // Enabled state as part of the attach process.
            if ped == true && pr == false && pls == 0 {
                // The attached USB3 device shall be in the Default statue.
                // Valid Port ID values are 1, 2, 3, ... MaxPorts).
                self.add_port(idx as u8 + 1, port_speed);
            }

            // A USB2 protocol port requires software to reset the port to advance
            // the port to the Enabled state and a USB device from the Powered
            // state to the Default state. After an attach event, the PED and PR flags
            // shall be ‘0’ and the PLS field shall be ‘7’ (Polling) in the PORTSC
            // register.
            if ped == false && pr == false && pls == 7 {
                // Enable the port by resetting the port (writing a '1' to the PORTSC PR bit)
                // then waiting for a Port Status Change Event.
                port.portsc.set_port_reset();
                self.regs
                    .lock()
                    .port_register_set
                    .write_volatile_at(idx, port);
            }

            // FIXME: how to deal with other state?
        }
    }

    fn add_port(&mut self, port_id: u8, port_speed: u8) {
        let regs = Arc::downgrade(&self.regs);
        let cmd_ring = Arc::downgrade(&self.cmd_ring);
        let new_slot = XhciSlot::init(
            port_id,
            port_speed,
            self.dev_ctxs.is_64bytes_context(),
            regs,
            cmd_ring,
        );
        for slot in self.slots.iter_mut() {
            if slot.is_none() {
                let _ = slot.insert(Arc::new(new_slot));
                break;
            }
        }

        let enable_slot = command::EnableSlot::default();
        let allowed = command::Allowed::EnableSlot(enable_slot);
        self.add_command(allowed).unwrap();
        self.ring_doorbell_at(0, 0, 0);
    }

    fn remove_port(&mut self, port_id: u8) {
        let mut removed = None;
        for slot in self.slots.iter_mut() {
            if slot.as_mut().is_some_and(|s| s.port_id() == port_id) {
                removed = slot.take();
                break;
            }
        }
        let Some(slot) = removed else {
            return;
        };
        self.dev_ctxs.clear_slot(&slot);

        let mut disable_slot = command::DisableSlot::default();
        disable_slot.set_slot_id(slot.slot_id());
        let allowed = command::Allowed::DisableSlot(disable_slot);
        self.add_command(allowed).unwrap();
        self.ring_doorbell_at(0, 0, 0);
    }

    pub fn is_command_ring_running(&self) -> bool {
        self.regs
            .lock()
            .operational
            .crcr
            .read_volatile()
            .command_ring_running()
    }

    pub fn register_class_driver(&mut self, class: Arc<dyn UsbClass>) {
        for slot in self.slots.iter() {
            let Some(slot) = slot else {
                continue;
            };

            let weak_slot = Arc::downgrade(slot);
            if class.probe(weak_slot.clone()).is_err() {
                continue;
            }

            class.init(weak_slot);
        }
    }
}

fn default_primary_irq_callback(trap_frame: &TrapFrame) {
    debug!(
        "xHCI primary irq callback, trap_num:{}",
        trap_frame.trap_num
    );
    let mut xhci_devices = XHCI_DRIVER.get().unwrap().controllers.disable_irq().lock();
    for dev in xhci_devices.iter_mut() {
        if dev.irq_num(0).unwrap() != trap_frame.trap_num {
            continue;
        }

        return dev.handle_primary_irq();
    }
}
