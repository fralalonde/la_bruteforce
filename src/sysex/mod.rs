use crate::sysex::universal::Universal;

pub mod arturia;
pub mod universal;

#[repr(C)]
pub struct Message {
    /// Always 0xF0
    header: Header,

    /// Manufacturer code, 1 byte or 3 bytes long if first byte is 0x00
    // TODO vendor table
    vendor_id: Vendor,

    /// Always 0xF7
    footer: Footer
}

impl Message {
    pub unsafe fn as_slice(&self) -> &[u8] {
        ::std::slice::from_raw_parts(
            (self as *const Self) as *const u8,
            ::std::mem::size_of::<Self>(),
        )
    }
}

impl Default for Message {
    fn default() -> Self {
        Message {
            header: Header::Start,
            vendor_id: Vendor::RealTime(universal::Universal::default()),
            footer: Footer::End
        }
    }
}

#[repr(u8)]
pub enum Header {
    Start = 0xF0
}

#[repr(u8)]
pub enum Footer {
    End = 0xF7
}

#[repr(u8)]
pub enum Vendor {
    RealTime(universal::Universal) = 0x7e,
    NonRealTime(universal::Universal) = 0x7f,
    Extended(VendorEx) = 0x00,
    Roland = 0x41,
    // ...
}

#[repr(u16)]
pub enum VendorEx {
    Arturia(arturia::Payload) = 0x206b,
    // ...
}

