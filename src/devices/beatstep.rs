use crate::devices::ParameterBounds::*;
use crate::devices::{Device, Param};

pub fn beatstep() -> Device {
    Device {
        name: "BeatStep",
        port_name: "Arturia BeatStep",
        usb_vendor_id: 0x1c75,
        usb_product_id: 0x0206,
        sysex_out_id: 0x05,
        sysex_tx_id: 0x06,
        params: vec![],
    }
}
