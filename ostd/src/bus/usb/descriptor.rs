/// Descriptor are used to determine the type of descriptor being queried
/// from a device or being set to a device.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Descriptor {
    Device = 0,
    Configuration,
    String,
    Interface,
    Endpoint,
    Reserved0,
    Reserved1,
    InterfacePower,
    Otg,
    Debug,
    InterfaceAssociation,
    Bos,
    DeviceCapability,
    SuperSpeedUsbEndpointCompanion,
    SuperSpeedPlusIsochronousEndpointCompanion,
}

impl Descriptor {
    pub fn type_id(&self) -> u8 {
        match self {
            Descriptor::Device => 1,
            Descriptor::Configuration => 2,
            Descriptor::String => 3,
            Descriptor::Interface => 4,
            Descriptor::Endpoint => 5,
            Descriptor::Reserved0 => 6,
            Descriptor::Reserved1 => 7,
            Descriptor::InterfacePower => 8,
            Descriptor::Otg => 9,
            Descriptor::Debug => 10,
            Descriptor::InterfaceAssociation => 11,
            Descriptor::Bos => 15,
            Descriptor::DeviceCapability => 16,
            Descriptor::SuperSpeedUsbEndpointCompanion => 48,
            Descriptor::SuperSpeedPlusIsochronousEndpointCompanion => 49,
        }
    }

    pub fn length(&self) -> usize {
        match self {
            Descriptor::Device => size_of::<DeviceDescriptor>(),
            Descriptor::Configuration => size_of::<ConfigurationDescriptor>(),
            Descriptor::InterfaceAssociation => size_of::<InterfaceAssociationDescriptor>(),
            Descriptor::Interface => size_of::<InterfaceDescriptor>(),
            Descriptor::Endpoint => size_of::<EndpointDescriptor>(),
            _ => todo!("implement more Descriptor"),
        }
    }
}

#[derive(Debug)]
#[repr(packed)]
pub struct DeviceDescriptor {
    // Size of this descriptor in bytes
    length: u8,
    // DEVICE Descriptor Type
    typ: u8,
    // USB Specification Release Number in Binary-Coded Decimal
    usb_bcd: u16,
    // Class code (assigned by the USB-IF)
    class: u8,
    // Subclass code (assigned by the USB-IF)
    sub_class: u8,
    // Protocol code (assigned by the USB-IF)
    protocol: u8,
    // Maximum packet size for endpoint zero
    pub max_packet_size: u8,
    // Vendor ID (assigned by the USB-IF)
    vendor_id: u16,
    // Product ID (assigned by the manufacturer)
    product_id: u16,
    // Device release number in binary-coded decimal
    device_bcd: u16,
    // Index of string descriptor describing manufacturer
    manufacturer_index: u8,
    // Index of string descriptor describing product
    product_index: u8,
    // Index of string descriptor describing the device’s serial number
    serial_number_index: u8,
    // Number of possible configurations
    nr_configurations: u8,
}

#[derive(Debug)]
#[repr(packed)]
pub struct ConfigurationDescriptor {
    // Size of this descriptor in bytes
    length: u8,
    // CONFIGURATION Descriptor Type
    typ: u8,
    // Total length of data returned for this configuration
    total_length: u16,
    // Number of interfaces supported by this configuration
    nr_interfaces: u8,
    // Value to use as an argument to the SetConfiguration() request to select this configuration
    value: u8,
    // Index of string descriptor describing this configuration
    index: u8,
    // Configuration characteristics
    attributes: u8,
    // Maximum power consumption of the device from the bus in this specific configuration
    // when the device is fully operational
    max_power: u8,
}

#[derive(Debug)]
#[repr(packed)]
pub struct InterfaceAssociationDescriptor {
    // Size of this descriptor in bytes
    length: u8,
    // INTERFACE ASSOCIATION Descriptor
    typ: u8,
    // Interface number of the first interface that is associated with this function
    first_interface: u8,
    // Number of contiguous interfaces that are associated with this function
    interface_conut: u8,
    // Class code (assigned by USB-IF)
    class: u8,
    // Subclass code (assigned by USB-IF)
    subclass: u8,
    // Protocol code (assigned by USB-IF)
    protocol: u8,
    // Index of string descriptor describing this function
    index: u8,
}

#[derive(Debug)]
#[repr(packed)]
pub struct InterfaceDescriptor {
    // Size of this descriptor in bytes
    length: u8,
    // INTERFACE Descriptor Type
    typ: u8,
    // Number of this interface
    number: u8,
    // Value used to select this alternate setting for the interface identified in the prior field
    alternate_setting: u8,
    // Number of endpoints used by this interface (excluding the Default Control Pipe)
    nr_endpoints: u8,
    // Class code (assigned by the USB-IF)
    class: u8,
    // Subclass code (assigned by the USB-IF)
    subclass: u8,
    // Protocol code (assigned by the USB-IF)
    protocol: u8,
    // Index of string descriptor describing this interface
    index: u8,
}

#[derive(Debug)]
#[repr(packed)]
pub struct EndpointDescriptor {
    // Size of this descriptor in bytes
    length: u8,
    // ENDPOINT Descriptor Type
    typ: u8,
    // The address of the endpoint on the device described by this descriptor
    address: u8,
    // The endpoint’s attributes when it is configured using the bConfigurationValue
    attributes: u8,
    // Maximum packet size this endpoint is capable
    max_packet_size: u16,
    // Interval for servicing the endpoint for data transfers
    interval: u8,
}
