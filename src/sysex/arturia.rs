#[repr(C)]

pub struct Payload {
    device_id: DeviceId,
    reserved1: u8,
    seq_count: u8,
    operation: Operation,
}

#[repr(u8)]
pub enum DeviceId {
    MicroBrute = 0x05
}

pub enum Operation {
    Update {
        exchange: Terminal,
        param_id: u8,
        value: u8,
    },
    Query {
        exchange: Initial,
        param_id: u8,
    },
    Answer {
        exchange: Terminal,
        param_id: u8,
        value: u8,
    },
}

#[repr(u8)]
pub enum Initial {
    Initial = 0x00,
}

#[repr(u8)]
pub enum Terminal {
    Terminal = 0x01,
}
