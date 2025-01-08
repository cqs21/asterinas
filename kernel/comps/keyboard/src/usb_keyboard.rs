// SPDX-License-Identifier: MPL-2.0

//! The usb hid keyboard driver.

use core::sync::atomic::{AtomicBool, Ordering};

use num_traits::FromPrimitive;
use ostd::bus::usb;

use super::{InputKey, KEYBOARD_CALLBACKS};

pub fn init() {
    usb::register_callback(&handle_usb_keyboard);
}

fn handle_usb_keyboard(report: &usb::KeyboardReport) {
    static HAS_MODIFIER: AtomicBool = AtomicBool::new(false);
    static HAS_ARROWS: AtomicBool = AtomicBool::new(false);

    let modifier = FromPrimitive::from_u8(report.modifier).unwrap_or(Modifier::Unknown);

    if HAS_MODIFIER.load(Ordering::Acquire) && modifier == Modifier::None {
        HAS_MODIFIER.store(false, Ordering::Release);
        return;
    }

    if modifier != Modifier::None {
        HAS_MODIFIER.store(true, Ordering::Release);
    }

    if report.keys[1] != 0x00 {
        return;
    }

    let key = FromPrimitive::from_u8(report.keys[0]).unwrap_or(Key::Reserved);

    if key == Key::None
        || key == Key::PostFail
        || key == Key::ErrorRollOver
        || key == Key::ErrorUndefined
    {
        return;
    }

    let has_arrows = HAS_ARROWS.load(Ordering::Acquire);
    if has_arrows
        && (key == Key::UpArrow
            || key == Key::DownArrow
            || key == Key::LeftArrow
            || key == Key::RightArrow)
    {
        HAS_ARROWS.store(false, Ordering::Release);
        return;
    }
    if key == Key::UpArrow
        || key == Key::DownArrow
        || key == Key::LeftArrow
        || key == Key::RightArrow
    {
        HAS_ARROWS.store(true, Ordering::Release);
    }

    let key = parse_inputkey(modifier, key);
    for callback in KEYBOARD_CALLBACKS.lock().iter() {
        callback(key);
    }
}

fn parse_inputkey(modifier: Modifier, key: Key) -> InputKey {
    match modifier {
        Modifier::LeftCtrl | Modifier::RightCtrl => key.ctrl_map(),
        Modifier::LeftShift | Modifier::RightShift => key.shift_map(),
        _ => key.plain_map(),
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
enum Modifier {
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

/// Defines a USB HID Key.
///
/// See the HID Usage Tables: <https://www.usb.org/sites/default/files/hut1_5.pdf>.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
enum Key {
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
    UnderscoreMinus = 0x2D,
    PlusEqual = 0x2E,
    LeftBraceBracket = 0x2F,
    RightBraceBracket = 0x30,
    PipeBackslash = 0x31,
    // Non-US # and ~
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
    // Deletes one character without changing position.
    DeleteForward = 0x4C,
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
    KeypadNonUSSlash = 0x64,
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

impl Key {
    fn plain_map(&self) -> InputKey {
        match self {
            Key::Aa => InputKey::LowercaseA,
            Key::Bb => InputKey::LowercaseB,
            Key::Cc => InputKey::LowercaseC,
            Key::Dd => InputKey::LowercaseD,
            Key::Ee => InputKey::LowercaseE,
            Key::Ff => InputKey::LowercaseF,
            Key::Gg => InputKey::LowercaseG,
            Key::Hh => InputKey::LowercaseH,
            Key::Ii => InputKey::LowercaseI,
            Key::Jj => InputKey::LowercaseJ,
            Key::Kk => InputKey::LowercaseK,
            Key::Ll => InputKey::LowercaseL,
            Key::Mm => InputKey::LowercaseM,
            Key::Nn => InputKey::LowercaseN,
            Key::Oo => InputKey::LowercaseO,
            Key::Pp => InputKey::LowercaseP,
            Key::Qq => InputKey::LowercaseQ,
            Key::Rr => InputKey::LowercaseR,
            Key::Ss => InputKey::LowercaseS,
            Key::Tt => InputKey::LowercaseT,
            Key::Uu => InputKey::LowercaseU,
            Key::Vv => InputKey::LowercaseV,
            Key::Ww => InputKey::LowercaseW,
            Key::Xx => InputKey::LowercaseX,
            Key::Yy => InputKey::LowercaseY,
            Key::Zz => InputKey::LowercaseZ,
            Key::Exclamation1 => InputKey::One,
            Key::At2 => InputKey::Two,
            Key::Hash3 => InputKey::Three,
            Key::Dollar4 => InputKey::Four,
            Key::Percent5 => InputKey::Five,
            Key::Caret6 => InputKey::Six,
            Key::Ampersand7 => InputKey::Seven,
            Key::Asterisk8 => InputKey::Eight,
            Key::LeftParen9 => InputKey::Nine,
            Key::RightParen0 => InputKey::Zero,
            Key::Enter => InputKey::Cr,
            Key::Escape => InputKey::Esc,
            Key::Backspace => InputKey::Del,
            Key::Tab => InputKey::Tab,
            Key::Space => InputKey::Space,
            Key::UnderscoreMinus => InputKey::Minus,
            Key::PlusEqual => InputKey::Equal,
            Key::LeftBraceBracket => InputKey::LeftBracket,
            Key::RightBraceBracket => InputKey::RightBracket,
            Key::PipeBackslash => InputKey::BackSlash,
            Key::NonUSHash => InputKey::Hash,
            Key::ColonSemicolon => InputKey::SemiColon,
            Key::DoubleSingleQuote => InputKey::SingleQuote,
            Key::TildeBacktick => InputKey::Backtick,
            Key::LessThanComma => InputKey::Comma,
            Key::GreaterThanPeriod => InputKey::Period,
            Key::QuestionSlash => InputKey::ForwardSlash,
            Key::F1 => InputKey::F1,
            Key::F2 => InputKey::F2,
            Key::F3 => InputKey::F3,
            Key::F4 => InputKey::F4,
            Key::F5 => InputKey::F5,
            Key::F6 => InputKey::F6,
            Key::F7 => InputKey::F7,
            Key::F8 => InputKey::F8,
            Key::F9 => InputKey::F9,
            Key::F10 => InputKey::F10,
            Key::F11 => InputKey::F11,
            Key::F12 => InputKey::F12,
            Key::Insert => InputKey::Insert,
            Key::Home => InputKey::Home,
            Key::PageUp => InputKey::PageUp,
            Key::DeleteForward => InputKey::Delete,
            Key::End => InputKey::End,
            Key::PageDown => InputKey::PageDown,
            Key::RightArrow => InputKey::RightArrow,
            Key::LeftArrow => InputKey::LeftArrow,
            Key::DownArrow => InputKey::DownArrow,
            Key::UpArrow => InputKey::UpArrow,
            Key::KeypadDivide => InputKey::ForwardSlash,
            Key::KeypadMultiply => InputKey::Asterisk,
            Key::KeypadMinus => InputKey::Minus,
            Key::KeypadPlus => InputKey::Plus,
            Key::KeypadEnter => InputKey::Cr,
            Key::Keypad1End => InputKey::One,
            Key::Keypad2DownArrow => InputKey::Two,
            Key::Keypad3PageDown => InputKey::Three,
            Key::Keypad4LeftArrow => InputKey::Four,
            Key::Keypad5 => InputKey::Five,
            Key::Keypad6RightArrow => InputKey::Six,
            Key::Keypad7Home => InputKey::Seven,
            Key::Keypad8UpArrow => InputKey::Eight,
            Key::Keypad9PageUp => InputKey::Nine,
            Key::Keypad0Insert => InputKey::Zero,
            Key::KeypadDotDelete => InputKey::Period,
            Key::KeypadEqual => InputKey::Equal,
            _ => InputKey::Nul,
        }
    }

    fn shift_map(&self) -> InputKey {
        match self {
            Key::Aa => InputKey::UppercaseA,
            Key::Bb => InputKey::UppercaseB,
            Key::Cc => InputKey::UppercaseC,
            Key::Dd => InputKey::UppercaseD,
            Key::Ee => InputKey::UppercaseE,
            Key::Ff => InputKey::UppercaseF,
            Key::Gg => InputKey::UppercaseG,
            Key::Hh => InputKey::UppercaseH,
            Key::Ii => InputKey::UppercaseI,
            Key::Jj => InputKey::UppercaseJ,
            Key::Kk => InputKey::UppercaseK,
            Key::Ll => InputKey::UppercaseL,
            Key::Mm => InputKey::UppercaseM,
            Key::Nn => InputKey::UppercaseN,
            Key::Oo => InputKey::UppercaseO,
            Key::Pp => InputKey::UppercaseP,
            Key::Qq => InputKey::UppercaseQ,
            Key::Rr => InputKey::UppercaseR,
            Key::Ss => InputKey::UppercaseS,
            Key::Tt => InputKey::UppercaseT,
            Key::Uu => InputKey::UppercaseU,
            Key::Vv => InputKey::UppercaseV,
            Key::Ww => InputKey::UppercaseW,
            Key::Xx => InputKey::UppercaseX,
            Key::Yy => InputKey::UppercaseY,
            Key::Zz => InputKey::UppercaseZ,
            Key::Exclamation1 => InputKey::Exclamation,
            Key::At2 => InputKey::At,
            Key::Hash3 => InputKey::Hash,
            Key::Dollar4 => InputKey::Dollar,
            Key::Percent5 => InputKey::Percent,
            Key::Caret6 => InputKey::Caret,
            Key::Ampersand7 => InputKey::Ampersand,
            Key::Asterisk8 => InputKey::Asterisk,
            Key::LeftParen9 => InputKey::LeftParen,
            Key::RightParen0 => InputKey::RightParen,
            Key::Enter => InputKey::Cr,
            Key::Escape => InputKey::Esc,
            Key::Backspace => InputKey::Del,
            Key::Tab => InputKey::Tab,
            Key::Space => InputKey::Space,
            Key::UnderscoreMinus => InputKey::Underscore,
            Key::PlusEqual => InputKey::Plus,
            Key::LeftBraceBracket => InputKey::LeftBrace,
            Key::RightBraceBracket => InputKey::RightBrace,
            Key::PipeBackslash => InputKey::Pipe,
            Key::NonUSHash => InputKey::Tilde,
            Key::ColonSemicolon => InputKey::Colon,
            Key::DoubleSingleQuote => InputKey::DoubleQuote,
            Key::TildeBacktick => InputKey::Tilde,
            Key::LessThanComma => InputKey::LessThan,
            Key::GreaterThanPeriod => InputKey::GreaterThan,
            Key::QuestionSlash => InputKey::Question,
            _ => InputKey::Nul,
        }
    }

    fn ctrl_map(&self) -> InputKey {
        match self {
            Key::Aa => InputKey::Soh,
            Key::Bb => InputKey::Stx,
            Key::Cc => InputKey::Etx,
            Key::Dd => InputKey::Eot,
            Key::Ee => InputKey::Enq,
            Key::Ff => InputKey::Ack,
            Key::Gg => InputKey::Bel,
            Key::Hh => InputKey::Bs,
            Key::Ii => InputKey::Tab,
            Key::Jj => InputKey::Lf,
            Key::Kk => InputKey::Vt,
            Key::Ll => InputKey::Ff,
            Key::Mm => InputKey::Cr,
            Key::Nn => InputKey::So,
            Key::Oo => InputKey::Si,
            Key::Pp => InputKey::Dle,
            Key::Qq => InputKey::Dc1,
            Key::Rr => InputKey::Dc2,
            Key::Ss => InputKey::Dc3,
            Key::Tt => InputKey::Dc4,
            Key::Uu => InputKey::Nak,
            Key::Vv => InputKey::Syn,
            Key::Ww => InputKey::Etb,
            Key::Xx => InputKey::Can,
            Key::Yy => InputKey::Em,
            Key::Zz => InputKey::Sub,
            Key::Exclamation1 => InputKey::One,
            Key::At2 => InputKey::Nul,
            Key::Hash3 => InputKey::Esc,
            Key::Dollar4 => InputKey::Fs,
            Key::Percent5 => InputKey::Gs,
            Key::Caret6 => InputKey::Rs,
            Key::Ampersand7 => InputKey::Us,
            Key::Asterisk8 => InputKey::Del,
            Key::LeftParen9 => InputKey::Nine,
            Key::RightParen0 => InputKey::Zero,
            Key::Enter => InputKey::Cr,
            Key::Backspace => InputKey::Bs,
            Key::UnderscoreMinus => InputKey::Us,
            Key::PlusEqual => InputKey::Equal,
            Key::LeftBraceBracket => InputKey::Esc,
            Key::RightBraceBracket => InputKey::Gs,
            Key::PipeBackslash => InputKey::Fs,
            Key::ColonSemicolon => InputKey::SemiColon,
            Key::DoubleSingleQuote => InputKey::SingleQuote,
            Key::TildeBacktick => InputKey::Backtick,
            Key::LessThanComma => InputKey::Comma,
            Key::GreaterThanPeriod => InputKey::Period,
            Key::QuestionSlash => InputKey::Us,
            _ => InputKey::Nul,
        }
    }
}
