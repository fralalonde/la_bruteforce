use crate::sysex::VendorEx;
use crate::sysex;

pub type Channel = u8;
pub type Version = u32;

const ALL_DEVICES: Channel = 0x7f;

#[repr(C)]
pub struct Universal {
    channel: Channel,
    sub_id: SubId,
    footer: Footer,
}

impl Default for Universal {
    fn default() -> Self {
        Universal {
            channel: ALL_DEVICES,
            sub_id: SubId::IdentityRequest,
            footer: Footer::End
        }
    }
}

#[repr(u16)]
pub enum SubId {
    IdentityRequest = 0x0601,
    IdentityReply(VendorEx, u16, u16, Version) = 0x0602,
}

#[repr(u8)]
pub enum Footer {
    End = 0xf7
}
