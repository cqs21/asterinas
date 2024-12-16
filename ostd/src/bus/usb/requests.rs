use alloc::collections::VecDeque;

use xhci::ring::trb::transfer::{self, Allowed};

use super::descriptor::{Descriptor, LangId};
use crate::mm::HasDaddr;

/// Control request direction of USB traffic.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Host to device.
    Out = 0,
    /// Device to host.
    In = 1,
}

/// Control request type.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RequestType {
    /// Request is a USB standard request.
    Standard = 0,
    /// Request is intended for a USB class.
    Class = 1,
    /// Request is vendor-specific.
    Vendor = 2,
    /// Reserved.
    Reserved = 3,
}

/// Control request recipient.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Recipient {
    /// Request is intended for the entire device.
    Device = 0,
    /// Request is intended for an interface. Generally, the `index` field of the request specifies
    /// the interface number.
    Interface = 1,
    /// Request is intended for an endpoint. Generally, the `index` field of the request specifies
    /// the endpoint address.
    Endpoint = 2,
    /// None of the above.
    Other = 3,
    /// Reserved.
    Reserved,
    /// Vendor specific.
    Vendor = 31,
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeviceFeatureSelector {
    RemoteWakeup = 1,
    TestMode = 2,
    EnableU1 = 48,
    EnableU2 = 49,
    EnableLTM = 50,
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InterfaceFeatureSelector {
    Suspend = 0,
}

#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EndpointFeatureSelector {
    Halt = 0,
}

/// Standard Request Codes.
#[repr(u8)]
#[derive(Debug)]
enum RequestCode {
    /// Get status for the specified recipient.
    GetStatus = 0,
    /// Clear or disable a specific feature.
    ClearFeature = 1,
    /// Set or enable a specific feature.
    SetFeature = 3,
    /// Set the device address for all future device accesses.
    SetAddress = 5,
    /// Get specified descriptor if the descriptor exists.
    GetDescriptor = 6,
    /// Update existing descriptors or new descriptors may be added.
    SetDescriptor = 7,
    /// Get the current device configuration value.
    GetConfiguration = 8,
    /// Set the device configuration.
    SetConfiguration = 9,
    /// Get the selected alternate setting for the specified interface.
    GetInterface = 10,
    /// Select an alternate setting for the specified interface.
    SetInterface = 11,
    /// Set and then report an endpoint’s synchronization frame.
    SyncFrame = 12,
    /// Set both the U1 and U2 System Exit Latency and the U1 or U2 exit latency
    /// for all the links between a device and a root port on the host.
    SetSystemExitLatency = 48,
    /// Inform the device of the delay from the time a host transmits a packet
    /// to the time it is received by the device.
    SetIsochronousDelay = 49,
}

/// A control request to build TRBs.
#[derive(Debug)]
pub enum Request {
    /// Get status for the specified recipient.
    GetStatus(GetStatus),
    /// Clear or disable a specific feature.
    ClearFeature(ClearFeature),
    /// Set or enable a specific feature.
    SetFeature(SetFeature),
    /// Set the device address for all future device accesses.
    SetAddress(SetAddress),
    /// Get specified descriptor if the descriptor exists.
    GetDescriptor(GetDescriptor),
    /// Update existing descriptors or new descriptors may be added.
    SetDescriptor(SetDescriptor),
    /// Get the current device configuration value.
    GetConfiguration(GetConfiguration),
    /// Set the device configuration.
    SetConfiguration(SetConfiguration),
    /// Get the selected alternate setting for the specified interface.
    GetInterface(GetInterface),
    /// Select an alternate setting for the specified interface.
    SetInterface(SetInterface),
    /// Set and then report an endpoint’s synchronization frame.
    SyncFrame(SyncFrame),
    /// Set both the U1 and U2 System Exit Latency and the U1 or U2 exit latency
    /// for all the links between a device and a root port on the host.
    SetSystemExitLatency(SetSystemExitLatency),
    /// Inform the device of the delay from the time a host transmits a packet
    /// to the time it is received by the device.
    SetIsochronousDelay(SetIsochronousDelay),
}

impl Request {
    // Initialize a Setup Stage TD.
    fn setup(
        direction: Direction,
        typ: RequestType,
        recipient: Recipient,
        code: RequestCode,
        value: u16,
        index: u16,
        length: u16,
    ) -> Allowed {
        let mut setup = transfer::SetupStage::default();
        let request_type = (direction as u8) << 7 | (typ as u8) << 5 | (recipient as u8 & 0x1F);
        setup.set_request_type(request_type);
        setup.set_request(code as u8);
        setup.set_value(value);
        setup.set_index(index);
        setup.set_length(length);
        if length == 0 {
            setup.set_transfer_type(transfer::TransferType::No);
        } else if direction == Direction::Out {
            setup.set_transfer_type(transfer::TransferType::Out);
        } else {
            setup.set_transfer_type(transfer::TransferType::In);
        }
        setup.clear_interrupt_on_completion();
        transfer::Allowed::SetupStage(setup)
    }

    // Initialize the Data Stage TD.
    fn data(direction: Direction, buffer: u64, length: u32) -> Allowed {
        let mut data = transfer::DataStage::default();
        if direction == Direction::In {
            data.set_direction(transfer::Direction::In);
        } else {
            data.set_direction(transfer::Direction::Out);
        }
        data.set_data_buffer_pointer(buffer);
        data.set_trb_transfer_length(length);
        data.clear_chain_bit();
        data.clear_interrupt_on_completion();
        data.clear_immediate_data();
        transfer::Allowed::DataStage(data)
    }

    // Initialize a Status Stage TD.
    fn status(direction: Direction) -> Allowed {
        let mut status = transfer::StatusStage::default();
        if direction == Direction::In {
            status.set_direction();
        } else {
            status.clear_direction();
        }
        status.clear_chain_bit();
        status.set_interrupt_on_completion();
        transfer::Allowed::StatusStage(status)
    }
}

impl IntoIterator for Request {
    type Item = Allowed;
    type IntoIter = TrbIter;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Request::GetStatus(req) => req.into_trbs(),
            Request::ClearFeature(req) => req.into_trbs(),
            Request::SetFeature(req) => req.into_trbs(),
            Request::SetAddress(req) => req.into_trbs(),
            Request::GetDescriptor(req) => req.into_trbs(),
            Request::SetDescriptor(req) => req.into_trbs(),
            Request::GetConfiguration(req) => req.into_trbs(),
            Request::SetConfiguration(req) => req.into_trbs(),
            Request::GetInterface(req) => req.into_trbs(),
            Request::SetInterface(req) => req.into_trbs(),
            Request::SyncFrame(req) => req.into_trbs(),
            Request::SetSystemExitLatency(req) => req.into_trbs(),
            Request::SetIsochronousDelay(req) => req.into_trbs(),
        }
    }
}

pub struct TrbIter {
    trbs: VecDeque<Allowed>,
}

impl Iterator for TrbIter {
    type Item = Allowed;

    fn next(&mut self) -> Option<Self::Item> {
        self.trbs.pop_front()
    }
}

#[derive(Debug)]
pub struct GetStatus {
    recipient: Recipient,
    index: u16,
    buf_addr: usize,
}

impl GetStatus {
    pub fn new<T: HasDaddr>(recipient: Recipient, index: u16, buffer: T) -> Self {
        Self {
            recipient,
            index,
            buf_addr: buffer.daddr(),
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let mut index = self.index;
        if self.recipient == Recipient::Endpoint {
            index |= 0x80;
        }

        let setup = Request::setup(
            Direction::In,
            RequestType::Standard,
            self.recipient,
            RequestCode::GetStatus,
            0,
            index,
            2,
        );
        let data = Request::data(Direction::In, self.buf_addr as u64, 2);
        let status = Request::status(Direction::Out);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct ClearFeature {
    recipient: Recipient,
    index: u16,
    value: u16,
}

impl ClearFeature {
    pub fn new(recipient: Recipient, index: u16, value: u16) -> Self {
        Self {
            recipient,
            index,
            value,
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let mut index = self.index;
        if self.recipient == Recipient::Endpoint {
            index |= 0x80;
        }

        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            self.recipient,
            RequestCode::ClearFeature,
            self.value,
            index,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetFeature {
    recipient: Recipient,
    index: u16,
    value: u16,
}

impl SetFeature {
    pub fn new(recipient: Recipient, index: u16, value: u16) -> Self {
        Self {
            recipient,
            index,
            value,
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let mut index = self.index;
        if self.recipient == Recipient::Endpoint {
            index |= 0x80;
        }

        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            self.recipient,
            RequestCode::SetFeature,
            self.value,
            index,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetAddress {
    value: u16,
}

impl SetAddress {
    pub fn new(value: u16) -> Self {
        Self { value }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::SetAddress,
            self.value,
            0,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

#[derive(Debug)]
pub struct GetDescriptor {
    descriptor: Descriptor,
    index: u16,
    lang_id: u16,
    buf_addr: usize,
    length: u16,
}

impl GetDescriptor {
    pub fn new<T: HasDaddr>(
        descriptor: Descriptor,
        index: u16,
        lang_id: LangId,
        buffer: T,
        length: u16,
    ) -> Self {
        Self {
            descriptor,
            index,
            lang_id,
            buf_addr: buffer.daddr(),
            length,
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let value = (self.descriptor as u16) << 8 | self.index;
        let setup = Request::setup(
            Direction::In,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::GetDescriptor,
            value,
            self.lang_id,
            self.length,
        );
        let data = Request::data(Direction::In, self.buf_addr as u64, self.length as u32);
        let status = Request::status(Direction::Out);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetDescriptor {
    descriptor: Descriptor,
    index: u16,
    lang_id: u16,
    buf_addr: usize,
    length: u16,
}

impl SetDescriptor {
    pub fn new<T: HasDaddr>(
        descriptor: Descriptor,
        index: u16,
        lang_id: LangId,
        buffer: T,
        length: u16,
    ) -> Self {
        Self {
            descriptor,
            index,
            lang_id,
            buf_addr: buffer.daddr(),
            length,
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let value = (self.descriptor as u16) << 8 | self.index;
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::SetDescriptor,
            value,
            self.lang_id,
            self.length,
        );
        let data = Request::data(Direction::Out, self.buf_addr as u64, self.length as u32);
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct GetConfiguration {
    buf_addr: usize,
}

impl GetConfiguration {
    pub fn new<T: HasDaddr>(buffer: T) -> Self {
        Self {
            buf_addr: buffer.daddr(),
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::In,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::GetConfiguration,
            0,
            0,
            1,
        );
        let data = Request::data(Direction::In, self.buf_addr as u64, 1);
        let status = Request::status(Direction::Out);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetConfiguration {
    value: u16,
}

impl SetConfiguration {
    pub fn new(value: u16) -> Self {
        Self { value }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::SetConfiguration,
            self.value,
            0,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

#[derive(Debug)]
pub struct GetInterface {
    index: u16,
    buf_addr: usize,
}

impl GetInterface {
    pub fn new<T: HasDaddr>(index: u16, buffer: T) -> Self {
        Self {
            index,
            buf_addr: buffer.daddr(),
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::In,
            RequestType::Standard,
            Recipient::Interface,
            RequestCode::GetInterface,
            0,
            self.index,
            1,
        );
        let data = Request::data(Direction::In, self.buf_addr as u64, 1);
        let status = Request::status(Direction::Out);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetInterface {
    index: u16,
    value: u16,
}

impl SetInterface {
    pub fn new(index: u16, value: u16) -> Self {
        Self { index, value }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Interface,
            RequestCode::SetInterface,
            self.value,
            self.index,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

#[derive(Debug)]
pub struct SyncFrame {
    index: u16,
    buf_addr: usize,
}

impl SyncFrame {
    pub fn new<T: HasDaddr>(index: u16, buffer: T) -> Self {
        Self {
            index,
            buf_addr: buffer.daddr(),
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::In,
            RequestType::Standard,
            Recipient::Endpoint,
            RequestCode::SyncFrame,
            0,
            self.index | 0x80,
            2,
        );
        let data = Request::data(Direction::In, self.buf_addr as u64, 2);
        let status = Request::status(Direction::Out);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetSystemExitLatency {
    buf_addr: usize,
}

impl SetSystemExitLatency {
    pub fn new<T: HasDaddr>(buffer: T) -> Self {
        Self {
            buf_addr: buffer.daddr(),
        }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::SetSystemExitLatency,
            0,
            0,
            6,
        );
        let data = Request::data(Direction::Out, self.buf_addr as u64, 6);
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, data, status]),
        }
    }
}

#[derive(Debug)]
pub struct SetIsochronousDelay {
    value: u16,
}

impl SetIsochronousDelay {
    pub fn new(value: u16) -> Self {
        Self { value }
    }

    pub fn into_trbs(self) -> TrbIter {
        let setup = Request::setup(
            Direction::Out,
            RequestType::Standard,
            Recipient::Device,
            RequestCode::SetIsochronousDelay,
            self.value,
            0,
            0,
        );
        let status = Request::status(Direction::In);

        TrbIter {
            trbs: VecDeque::from([setup, status]),
        }
    }
}

// /// A trait for device-specific USB peripherals. Implement this to add support for
// /// different host controller.
// pub trait UsbController: Sync + Sized {
//     /// Allocates an endpoint and specified endpoint parameters. This method is called by the device
//     /// and class implementations to allocate endpoints.
//     fn alloc_endpoint(
//         &mut self,
//         ep_dir: Direction,
//         ep_addr: Option<EndpointAddress>,
//         ep_type: EndpointType,
//         max_packet_size: u16,
//         interval: u8,
//     ) -> Result<EndpointAddress>;

//     /// Enables and initializes the USB peripheral. Soon after enabling the device will be reset, so
//     /// there is no need to perform a USB reset in this method.
//     fn enable(&mut self);

//     /// Called when the host resets the device.
//     fn reset(&self);

//     /// Sets the device USB address to `addr`.
//     fn set_device_address(&self, addr: u8);

//     /// Writes a single packet of data to the specified endpoint and returns number of bytes
//     /// actually written.
//     fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> Result<usize>;

//     /// Reads a single packet of data from the specified endpoint and returns the actual length of
//     /// the packet.
//     fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> Result<usize>;

//     /// Sets or clears the STALL condition for an endpoint. If the endpoint is an OUT endpoint, it
//     /// should be prepared to receive data again.
//     fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool);

//     /// Gets whether the STALL condition is set for an endpoint.
//     fn is_stalled(&self, ep_addr: EndpointAddress) -> bool;

//     /// Causes the USB peripheral to enter USB suspend mode, lowering power consumption and
//     /// preparing to detect a USB wakeup event. This will be called after
//     /// [`poll`](crate::device::UsbDevice::poll) returns [`PollResult::Suspend`]. The device will
//     /// continue be polled, and it shall return a value other than `Suspend` from `poll` when it no
//     /// longer detects the suspend condition.
//     fn suspend(&self);

//     /// Resumes from suspend mode. This may only be called after the peripheral has been previously
//     /// suspended.
//     fn resume(&self);

//     /// Registers a Transfer Event handler for any information about events and incoming data.
//     /// Usually called in a loop or from an interrupt handler.
//     fn register_event_handler(&self, callback: &'static (dyn Fn(TransferEvent) + Send + Sync));
// }

// /// A trait for implementing USB classes.
// pub trait UsbClass<C: UsbController> {
//     /// Called when a GET_DESCRIPTOR request is received for a configuration descriptor. When
//     /// called, the implementation should write its interface, endpoint and any extra class
//     /// descriptors into `writer`.
//     fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()>;

//     /// Called when a GET_DESCRIPTOR request is received for a BOS descriptor.
//     /// When called, the implementation should write its blobs such as capability
//     /// descriptors into `writer`.
//     fn get_bos_descriptors(&self, writer: &mut BosWriter) -> Result<()>;

//     /// Gets a class-specific string descriptor.
//     ///
//     /// All string descriptor requests are passed to all classes in turn, so implementations
//     /// should return `None` if an unknown index is requested.
//     fn get_string(&self, index: u16, lang_id: u16) -> Option<&str>;

//     /// Called when a control request is received with direction HostToDevice.
//     fn control_out(&mut self, handler: ControlOut<C>);

//     /// Called when a control request is received with direction DeviceToHost.
//     fn control_in(&mut self, handler: ControlIn<C>);

//     /// Called when endpoint with address `addr` has received data (OUT packet).
//     fn endpoint_out(&mut self, addr: EndpointAddress);

//     /// Called when endpoint with address `addr` has completed transmitting data (IN packet).
//     fn endpoint_in(&mut self, addr: EndpointAddress);

//     /// Called when the interfaces alternate setting state is requested.
//     fn get_alt_setting(&mut self, interface: InterfaceNumber) -> Option<u8>;

//     /// Called when the interfaces alternate setting state is altered.
//     fn set_alt_setting(&mut self, interface: InterfaceNumber, alternative: u8) -> bool;
// }
