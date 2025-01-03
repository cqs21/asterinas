use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
    vec::Vec,
};

use spin::Once;
use xhci::{
    context::{EndpointState, SlotState},
    ring::trb::{
        event::{CommandCompletion, TransferEvent},
        transfer,
    },
};

use super::{
    slot::{CommandCompletionCallback, TransferEventCallback},
    BusProbeError, DeviceDescriptor, EndpointDescriptor, InterfaceDescriptor, Request,
};
use crate::{
    bus::usb::USB_KBD,
    mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions},
    sync::SpinLock,
};

pub trait UsbDevice: Send + Sync {
    fn device(&self) -> DeviceDescriptor;
    fn slot_state(&self) -> SlotState;
    fn interfaces(&self) -> Vec<InterfaceDescriptor>;
    fn endpoints(&self) -> Vec<EndpointDescriptor>;
    fn enable_endpoint(&self, endpoint: &EndpointDescriptor);
    fn endpoint_state(&self, endpoint: &EndpointDescriptor) -> EndpointState;
    fn send_device_request(&self, request: Request);
    fn send_endpoint_request(&self, endpoint: &EndpointDescriptor, allowed: transfer::Allowed);
    fn register_completion_callback(&self, callback: Box<CommandCompletionCallback>);
    fn register_event_callback(&self, callback: Box<TransferEventCallback>);
}

pub trait UsbClass {
    fn probe(&self, device: Weak<dyn UsbDevice>) -> Result<(), BusProbeError>;
    fn init(&self, device: Weak<dyn UsbDevice>);
}

pub struct UsbKeyboardDriver {
    devices: SpinLock<Vec<Weak<dyn UsbDevice>>>,
    callbacks: SpinLock<Vec<Box<dyn Fn(&KeyboardReport) + Send + Sync>>>,
    dma_buffer: DmaCoherent,
}

impl UsbKeyboardDriver {
    pub fn new() -> Self {
        let seg = FrameAllocOptions::new().alloc_segment(1).unwrap();
        let dma_buffer = DmaCoherent::map(seg.into(), true).unwrap();

        Self {
            devices: SpinLock::new(Vec::new()),
            callbacks: SpinLock::new(Vec::new()),
            dma_buffer,
        }
    }

    pub fn send_normal(&self) {
        let mut normal = transfer::Normal::default();
        normal.set_data_buffer_pointer(self.dma_buffer.start_paddr() as u64);
        normal.set_trb_transfer_length(8);
        normal.set_interrupt_on_completion();
        normal.clear_chain_bit();
        let allowed = transfer::Allowed::Normal(normal);

        let devices = self.devices.lock();
        for device in devices.iter() {
            let Some(device) = device.upgrade() else {
                continue;
            };

            for endpoint in device.endpoints().iter() {
                device.send_endpoint_request(endpoint, allowed.clone());
            }
        }
    }

    pub fn dma_buffer(&self) -> &KeyboardReport {
        let va = paddr_to_vaddr(self.dma_buffer.start_paddr());
        unsafe { &*(va as *const KeyboardReport) }
    }

    pub fn register_callback(&self, callback: &'static (dyn Fn(&KeyboardReport) + Send + Sync)) {
        self.callbacks.lock().push(Box::new(callback));
    }

    pub fn handle_events(&self) {
        let callbacks = self.callbacks.disable_irq().lock();
        for callback in callbacks.iter() {
            callback(self.dma_buffer());
        }

        self.send_normal();
    }
}

impl UsbClass for UsbKeyboardDriver {
    fn probe(&self, device: Weak<dyn UsbDevice>) -> Result<(), BusProbeError> {
        let Some(device) = device.upgrade() else {
            return Err(BusProbeError::DeviceNotMatch);
        };

        for interface in device.interfaces().iter() {
            if interface.class == 3 && interface.subclass == 1 && interface.protocol == 1 {
                return Ok(());
            }
        }

        return Err(BusProbeError::DeviceNotMatch);
    }

    fn init(&self, device: Weak<dyn UsbDevice>) {
        let Some(device) = device.upgrade() else {
            return;
        };

        device.register_completion_callback(Box::new(handle_command_completion));

        device.register_event_callback(Box::new(handle_transfer_event));

        for endpoint in device.endpoints().iter() {
            device.enable_endpoint(endpoint);
        }

        self.devices.lock().push(Arc::downgrade(&device));
    }
}

fn handle_command_completion(completion: &CommandCompletion) {
    let Some(kbd) = USB_KBD.get() else {
        return;
    };

    kbd.handle_events();
}

fn handle_transfer_event(event: &TransferEvent) {
    let Some(kbd) = USB_KBD.get() else {
        return;
    };

    kbd.handle_events();
}

pub fn register_callback(callback: &'static (dyn Fn(&KeyboardReport) + Send + Sync)) {
    let Some(kbd) = USB_KBD.get() else {
        return;
    };

    kbd.register_callback(callback);
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    None = 0x00,
    LeftCtrl = 0x01,
    LeftShift = 0x02,
    LeftAlt = 0x04,
    // Windowing environment key, examples are Microsoft Left Win key, Mac Left Apple key, Sun Left Meta key
    LeftMeta = 0x08,
    RightCtrl = 0x10,
    RightShift = 0x20,
    RightAlt = 0x40,
    // Windowing environment key, examples are Microsoft®RIGHT WIN key, Macintosh®RIGHT APPLE key, Sun®RIGHT META key.
    RightMeta = 0x80,
    Unknown,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    None = 0x00,
    // Error Roll Over - used for all slots if too many keys are pressed.
    ErrorRollOver = 0x01,
    PostFail = 0x02,
    ErrorUndefined = 0x03,
    Aa = 0x04,
    Bb = 0x05,
    Cc = 0x06,
    Dd = 0x07,
    Ee = 0x08,
    Ff = 0x09,
    Gg = 0x0A,
    Hh = 0x0B,
    Ii = 0x0C,
    Jj = 0x0D,
    Kk = 0x0E,
    Ll = 0x0F,
    Mm = 0x10,
    Nn = 0x11,
    Oo = 0x12,
    Pp = 0x13,
    Qq = 0x14,
    Rr = 0x15,
    Ss = 0x16,
    Tt = 0x17,
    Uu = 0x18,
    Vv = 0x19,
    Ww = 0x1A,
    Xx = 0x1B,
    Yy = 0x1C,
    Zz = 0x1D,
    Exclamation1 = 0x1E,
    At2 = 0x1F,
    Hash3 = 0x20,
    Dollar4 = 0x21,
    Percent5 = 0x22,
    Caret6 = 0x23,
    Ampersand7 = 0x24,
    Asterisk8 = 0x25,
    LeftParen9 = 0x26,
    RightParen0 = 0x27,
    Enter = 0x28,
    Escape = 0x29,
    Backspace = 0x2A,
    Tab = 0x2B,
    Space = 0x2C,
    MinusDash = 0x2D,
    PlusEqual = 0x2E,
    LeftBraceBracket = 0x2F,
    RightBraceBracket = 0x30,
    PipeBackslash = 0x31,
    NonUSHash = 0x32,
    ColonSemicolon = 0x33,
    DoubleSingleQuote = 0x34,
    TildeBacktick = 0x35,
    LessThanComma = 0x36,
    GreaterThanPeriod = 0x37,
    QuestionSlash = 0x38,
    CapsLock = 0x39,
    F1 = 0x3A,
    F2 = 0x3B,
    F3 = 0x3C,
    F4 = 0x3D,
    F5 = 0x3E,
    F6 = 0x3F,
    F7 = 0x40,
    F8 = 0x41,
    F9 = 0x42,
    F10 = 0x43,
    F11 = 0x44,
    F12 = 0x45,
    PrintScreen = 0x46,
    ScrollLock = 0x47,
    Pause = 0x48,
    Insert = 0x49,
    Home = 0x4A,
    PageUp = 0x4B,
    Delete = 0x4C,
    End = 0x4D,
    PageDown = 0x4E,
    RightArrow = 0x4F,
    LeftArrow = 0x50,
    DownArrow = 0x51,
    UpArrow = 0x52,
    KeypadNumLock = 0x53,
    KeypadDivide = 0x54,
    KeypadMultiply = 0x55,
    KeypadMinus = 0x56,
    KeypadPlus = 0x57,
    KeypadEnter = 0x58,
    Keypad1End = 0x59,
    Keypad2DownArrow = 0x5A,
    Keypad3PageDown = 0x5B,
    Keypad4LeftArrow = 0x5C,
    Keypad5 = 0x5D,
    Keypad6RightArrow = 0x5E,
    Keypad7Home = 0x5F,
    Keypad8UpArrow = 0x60,
    Keypad9PageUp = 0x61,
    Keypad0Insert = 0x62,
    KeypadDotDelete = 0x63,
    NonUSSlash = 0x64,
    Application = 0x65,
    Power = 0x66,
    KeypadEqual = 0x67,
    F13 = 0x68,
    F14 = 0x69,
    F15 = 0x6A,
    F16 = 0x6B,
    F17 = 0x6C,
    F18 = 0x6D,
    F19 = 0x6E,
    F20 = 0x6F,
    F21 = 0x70,
    F22 = 0x71,
    F23 = 0x72,
    F24 = 0x73,
    Execute = 0x74,
    Help = 0x75,
    Menu = 0x76,
    Select = 0x77,
    Stop = 0x78,
    Again = 0x79,
    Undo = 0x7A,
    Cut = 0x7B,
    Copy = 0x7C,
    Paste = 0x7D,
    Find = 0x7E,
    Mute = 0x7F,
    VolumeUp = 0x80,
    VolumeDown = 0x81,
    // Implemented as a locking key; sent as a toggle button.
    // Available for legacy support; however, most systems should use the non-locking version of this key.
    LockingCapsLock = 0x82,
    LockingNumLock = 0x83,
    LockingScrollLock = 0x84,
    KeypadComma = 0x85,
    // Used on AS/400 keyboards.
    KeypadEqualSign = 0x86,
    International1 = 0x87,
    International2 = 0x88,
    International3 = 0x89,
    International4 = 0x8A,
    International5 = 0x8B,
    International6 = 0x8C,
    International7 = 0x8D,
    International8 = 0x8E,
    International9 = 0x8F,
    Lang1 = 0x90,
    Lang2 = 0x91,
    Lang3 = 0x92,
    Lang4 = 0x93,
    Lang5 = 0x94,
    Lang6 = 0x95,
    Lang7 = 0x96,
    Lang8 = 0x97,
    Lang9 = 0x98,
    AlternateErase = 0x99,
    SysReq = 0x9A,
    Cancel = 0x9B,
    Clear = 0x9C,
    Prior = 0x9D,
    Return = 0x9E,
    Separator = 0x9F,
    Out = 0xA0,
    Oper = 0xA1,
    ClearOrAgain = 0xA2,
    CrSelOrProps = 0xA3,
    ExSel = 0xA4,
    // Reserved = 0xA5 - 0xAF,
    Keypad00 = 0xB0,
    Keypad000 = 0xB1,
    ThousandsSeparator = 0xB2,
    DecimalSeparator = 0xB3,
    CurrencyUnit = 0xB4,
    CurrencySubUnit = 0xB5,
    KeypadLeftParen = 0xB6,
    KeypadRightParen = 0xB7,
    KeypadLeftBrace = 0xB8,
    KeypadRightBrace = 0xB9,
    KeypadTab = 0xBA,
    KeypadBackspace = 0xBB,
    KeypadA = 0xBC,
    KeypadB = 0xBD,
    KeypadC = 0xBE,
    KeypadD = 0xBF,
    KeypadE = 0xC0,
    KeypadF = 0xC1,
    KeypadBitwiseXor = 0xC2,
    KeypadLogicalXor = 0xC3,
    KeypadModulo = 0xC4,
    KeypadLeftShift = 0xC5,
    KeypadRightShift = 0xC6,
    KeypadBitwiseAnd = 0xC7,
    KeypadLogicalAnd = 0xC8,
    KeypadBitwiseOr = 0xC9,
    KeypadLogicalOr = 0xCA,
    KeypadColon = 0xCB,
    KeypadHash = 0xCC,
    KeypadSpace = 0xCD,
    KeypadAt = 0xCE,
    KeypadExclamation = 0xCF,
    KeypadMemoryStore = 0xD0,
    KeypadMemoryRecall = 0xD1,
    KeyPadMemoryClear = 0xD2,
    KeypadMemoryAdd = 0xD3,
    KeypadMemorySubtract = 0xD4,
    KeypadMemoryMultiply = 0xD5,
    KeypadMemoryDivide = 0xD6,
    KeypadPlusMinus = 0xD7,
    KeypadClear = 0xD8,
    KeypadClearEntry = 0xD9,
    KeypadBinary = 0xDA,
    KeypadOctal = 0xDB,
    KeypadDecimal = 0xDC,
    KeypadHexadecimal = 0xDD,
    // Reserved = 0xDE - 0xDF,
    LeftControl = 0xE0,
    LeftShift = 0xE1,
    LeftAlt = 0xE2,
    // Windowing environment key, examples are Microsoft Left Win key, Mac Left Apple key, Sun Left Meta key.
    LeftMeta = 0xE3,
    RightControl = 0xE4,
    RightShift = 0xE5,
    RightAlt = 0xE6,
    // Windowing environment key, examples are Microsoft®RIGHT WIN key, Macintosh®RIGHT APPLE key, Sun®RIGHT META key.
    RightMeta = 0xE7,
    Reserved,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KeyboardReport {
    pub modifier: Modifier,
    pub reserved: u8,
    pub keys: [Key; 6],
}
