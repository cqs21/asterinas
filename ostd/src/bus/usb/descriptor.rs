use alloc::{string::String, vec::Vec};

use ostd_pod::Pod;

use super::requests::Direction;

/// Descriptor are used to determine the requested type.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Descriptor {
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    Reserved0 = 6,
    Reserved1 = 7,
    InterfacePower = 8,
    Otg = 9,
    Debug = 10,
    InterfaceAssociation = 11,
    Bos = 15,
    DeviceCapability = 16,
    SuperSpeedUsbEndpointCompanion = 48,
    SuperSpeedPlusIsochronousEndpointCompanion = 49,
    Unknown,
}

impl From<u8> for Descriptor {
    fn from(value: u8) -> Self {
        match value {
            1 => Descriptor::Device,
            2 => Descriptor::Configuration,
            3 => Descriptor::String,
            4 => Descriptor::Interface,
            5 => Descriptor::Endpoint,
            6 => Descriptor::Reserved0,
            7 => Descriptor::Reserved1,
            8 => Descriptor::InterfacePower,
            9 => Descriptor::Otg,
            10 => Descriptor::Debug,
            11 => Descriptor::InterfaceAssociation,
            15 => Descriptor::Bos,
            16 => Descriptor::DeviceCapability,
            48 => Descriptor::SuperSpeedUsbEndpointCompanion,
            49 => Descriptor::SuperSpeedPlusIsochronousEndpointCompanion,
            _ => Descriptor::Unknown,
        }
    }
}

/// A device descriptor describes general information about a device.
/// It includes information that applies to the device and all the configurations.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct DeviceDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // DEVICE Descriptor Type
    pub typ: u8,
    // USB Specification Release Number in Binary-Coded Decimal
    pub usb_bcd: u16,
    // Class code (assigned by the USB-IF)
    pub class: u8,
    // Subclass code (assigned by the USB-IF)
    pub sub_class: u8,
    // Protocol code (assigned by the USB-IF)
    pub protocol: u8,
    // Maximum packet size for endpoint zero
    pub max_packet_size: u8,
    // Vendor ID (assigned by the USB-IF)
    pub vendor_id: u16,
    // Product ID (assigned by the manufacturer)
    pub product_id: u16,
    // Device release number in binary-coded decimal
    pub device_bcd: u16,
    // Index of string descriptor describing manufacturer
    pub manufacturer_index: u8,
    // Index of string descriptor describing product
    pub product_index: u8,
    // Index of string descriptor describing the device’s serial number
    pub serial_number_index: u8,
    // Number of possible configurations
    pub nr_configurations: u8,
}

/// A configuration descriptor describes information about a specific device configuration.
///
/// The descriptor contains a `value` field, when used as a parameter to the SetConfiguration
/// request, causes the device to assume the described configuration.
///
/// When the host requests the configuration descriptor, all related interface, endpoint, and
/// endpoint companion descriptors are returned. The `total_length` filed indicates the combined
/// length of all descriptors (configuration, interface, endpoint, and class- or vendor-specific)
/// returned for this configuration.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct ConfigurationDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // CONFIGURATION Descriptor Type
    pub typ: u8,
    // Total length of data returned for this configuration
    pub total_length: u16,
    // Number of interfaces supported by this configuration
    pub nr_interfaces: u8,
    // Value to use as an argument to the SetConfiguration() request to select this configuration
    pub value: u8,
    // Index of string descriptor describing this configuration
    pub index: u8,
    // Configuration characteristics
    pub attributes: u8,
    // Maximum power consumption of the device from the bus in this specific configuration
    // when the device is fully operational
    pub max_power: u8,
}

impl ConfigurationDescriptor {
    /// Returns true if the device configuration is self powered.
    ///
    /// A device configuration that uses power from the bus and a local source reports
    /// a non-zero value in `max_power` to indicate the amount of bus power required.
    /// The actual power source at runtime may be determined by GetStatus(DEVICE) request.
    pub fn is_self_powered(&self) -> bool {
        self.attributes & 0x40 != 0
    }

    /// Returns true if the device configuration supports remote wakeup.
    pub fn can_remote_wakeup(&self) -> bool {
        self.attributes & 0x20 != 0
    }
}

/// The Interface Association Descriptor is used to describe that two or more interfaces are
/// associated to the same function.
///
/// A device must use an Interface Association descriptor for each device function that requires
/// more than one interface. An Interface Association descriptor is always returned as part of
/// the configuration information returned by a GetDescriptor(Configuration) request, located
/// before the set of interface descriptors (including all alternate settings) for the interfaces
/// it associates.
///
/// An interface association descriptor cannot be directly accessed with a GetDescriptor or
/// SetDescriptor request.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct InterfaceAssociationDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // INTERFACE ASSOCIATION Descriptor
    pub typ: u8,
    // Interface number of the first interface that is associated with this function
    pub first_interface: u8,
    // Number of contiguous interfaces that are associated with this function
    pub interface_conut: u8,
    // Class code (assigned by USB-IF)
    pub class: u8,
    // Subclass code (assigned by USB-IF)
    pub subclass: u8,
    // Protocol code (assigned by USB-IF)
    pub protocol: u8,
    // Index of string descriptor describing this function
    pub index: u8,
}

/// The interface descriptor describes a specific interface within a configuration.
///
/// A configuration provides one or more interfaces, each with zero or more endpoint descriptors.
/// When a configuration supports more than one interface, the endpoint descriptors for a particular
/// interface follow the interface descriptor in the data returned by the GetConfiguration request.
///
/// An interface descriptor is always returned as part of a configuration descriptor, cannot be
/// directly accessed with a GetDescriptor or SetDescriptor request.
///
/// An interface may include alternate settings that allow the endpoints and/or their characteristics
/// to be varied after the device has been configured. The default setting for an interface is always
/// alternate setting zero. The SetInterface request is used to select an alternate setting or to
/// return to the default setting. The GetInterface request returns the selected alternate setting.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct InterfaceDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // INTERFACE Descriptor Type
    pub typ: u8,
    // Number of this interface
    pub number: u8,
    // Value used to select this alternate setting for the interface identified in the prior field
    pub alternate_setting: u8,
    // Number of endpoints used by this interface (excluding the Default Control Pipe)
    pub nr_endpoints: u8,
    // Class code (assigned by the USB-IF)
    pub class: u8,
    // Subclass code (assigned by the USB-IF)
    pub subclass: u8,
    // Protocol code (assigned by the USB-IF)
    pub protocol: u8,
    // Index of string descriptor describing this interface
    pub index: u8,
}

/// The endpoint descriptor describes a specific endpoint within a interface.
///
/// An endpoint descriptor is always returned as part of the configuration information
/// returned by a GetDescriptor(Configuration) request. An endpoint descriptor cannot be
/// directly accessed with a GetDescriptor or SetDescriptor request.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct EndpointDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // ENDPOINT Descriptor Type
    pub typ: u8,
    // The address of the endpoint on the device described by this descriptor
    pub address: u8,
    // The endpoint’s attributes when it is configured using the bConfigurationValue
    pub attributes: u8,
    // Maximum packet size this endpoint is capable
    pub max_packet_size: u16,
    // Interval for servicing the endpoint for data transfers
    pub interval: u8,
}

impl EndpointDescriptor {
    /// Returns the direction of the endpoint.
    pub fn direction(&self) -> Direction {
        if self.address >> 7 == 0 {
            Direction::Out
        } else {
            Direction::In
        }
    }

    /// Returns device context index (DCI) of the endpoint.
    pub fn device_context_index(&self) -> u8 {
        let number = self.address & 0xF;
        let direction = self.address >> 7; // 0 = OUT endpoint, 1 = IN endpoint
        (number * 2) + direction
    }
}

/// BOS descriptor defines a root descriptor for accessing a family of related descriptors.
///
/// The entire set can only be accessed via reading the BOS descriptor with a GetDescriptor
/// request and using the length reported in the `total_length` field. There is no way for
/// a host to read individual device capability descriptors.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct BosDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // BOS Descriptor Type
    pub typ: u8,
    // Length of this descriptor and all of its sub descriptors
    pub total_length: u16,
    // Number of separate device capability descriptors in the BOS
    pub nr_capabilities: u8,
}

/// String descriptors use UNICODE UTF16LE encodings as defined by The Unicode
/// Standard, Worldwide Character Encoding, Version 5.0.
///
/// The strings in a device may support multiple languages. When requesting a string
/// descriptor, the requester specifies the desired language using a 16-bit language
/// ID (LangId) defined by the USB-IF. The list of currently defined USB LangIds can
/// be found at <http://www.usb.org/developers/docs.html>.
///
/// String index zero for all languages returns a string descriptor that contains an
/// array of 2-byte LangId codes supported by the device. The array of LangId codes
/// is not NULL-terminated. The size of the array (in bytes) is computed by subtracting
/// two from the value of the first byte of the descriptor.
/// +------------+----------+---------------+-----+---------------+
/// | length(u8) | type(u8) | LangId_0(u16) | ... | LangId_N(u16) |
/// +------------+----------+---------------+-----+---------------+
///
/// The UNICODE string descriptor is not NULL-terminated. The string length N is computed
/// by subtracting two from the value of the first byte of the descriptor.
/// +------------+----------+-----------------+
/// | length(u8) | type(u8) | String(N bytes) |
/// +------------+----------+-----------------+
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
pub struct StringDescriptor {
    // Size of this descriptor in bytes
    pub length: u8,
    // String Descriptor Type
    pub typ: u8,
}

/// The language Id, e.g., 0x0409 represents English (United States).
pub type LangId = u16;

/// This struct is used to parse descriptor within an u8 slice.
#[derive(Clone, Debug)]
pub enum DescriptorWrapper {
    Device(DeviceDescriptor),
    Configuration(ConfigurationDescriptor),
    InterfaceAssociation(InterfaceAssociationDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
    Bos(BosDescriptor),
    String(String),
    Unknown(Vec<u8>),
}

#[derive(Debug)]
pub struct DescriptorIter<'a> {
    buffer: &'a [u8],
    cursor: usize,
}

impl<'a> DescriptorIter<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        if buffer.len() < 4 {
            return Self {
                buffer: &buffer[..0],
                cursor: 0,
            };
        }

        let total_length = match Descriptor::from(buffer[1]) {
            Descriptor::Bos | Descriptor::Configuration => {
                (buffer[3] as usize) << 8 | (buffer[2] as usize)
            }
            _ => buffer[0] as usize,
        };
        let len = buffer.len().min(total_length);

        Self {
            buffer: &buffer[..len],
            cursor: 0,
        }
    }
}

impl<'a> Iterator for DescriptorIter<'a> {
    type Item = DescriptorWrapper;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.cursor;
        if offset >= self.buffer.len() {
            return None;
        }
        let len = (self.buffer.len() - offset).min(self.buffer[offset] as usize);

        let wrapper = match Descriptor::from(self.buffer[self.cursor + 1]) {
            Descriptor::Device => {
                let mut dev = DeviceDescriptor::new_zeroed();
                dev.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Device(dev)
            }
            Descriptor::Configuration => {
                let mut conf = ConfigurationDescriptor::new_zeroed();
                conf.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Configuration(conf)
            }
            Descriptor::String => {
                let str = String::from_utf16le_lossy(&self.buffer[offset + 2..offset + len]);
                DescriptorWrapper::String(str)
            }
            Descriptor::Interface => {
                let mut inf = InterfaceDescriptor::new_zeroed();
                inf.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Interface(inf)
            }
            Descriptor::Endpoint => {
                let mut ep = EndpointDescriptor::new_zeroed();
                ep.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Endpoint(ep)
            }
            Descriptor::InterfaceAssociation => {
                let mut infa = InterfaceAssociationDescriptor::new_zeroed();
                infa.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::InterfaceAssociation(infa)
            }
            Descriptor::Bos => {
                let mut bos = BosDescriptor::new_zeroed();
                bos.as_bytes_mut()[..len].copy_from_slice(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Bos(bos)
            }
            _ => {
                let vec = Vec::from(&self.buffer[offset..offset + len]);
                DescriptorWrapper::Unknown(vec)
            }
        };

        self.cursor += len;
        Some(wrapper)
    }
}
