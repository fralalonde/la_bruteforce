use linked_hash_map::LinkedHashMap;
use std::iter::Iterator;

use crate::midi::MidiPort;

mod beatstep;
mod microbrute;

pub type MidiValue = u8;
pub type SysexParamId = u8;

#[derive(Debug)]
enum HotplugEvent {
    CONNECTED(&'static Device),
    DISCONNECTED(&'static Device),
}

#[derive(Debug)]
pub struct Device {
    usb_vendor_id: u32,
    usb_product_id: u32,
    sysex_out_id: u8,
    sysex_cmd_id: u8,
    port_name: &'static str,
    pub name: &'static str,
    pub params: Vec<Param>,
}

#[derive(Debug)]
pub struct Param {
    sysex_out_id: SysexParamId,
    sysex_cmd_id: SysexParamId,
    pub name: &'static str,
    pub bounds: ParameterBounds,
}

#[derive(Debug)]
pub enum ParameterBounds {
    Discrete(Vec<(MidiValue, &'static str)>),
    Range(MidiValue, MidiValue),
}

pub fn known_devices() -> Vec<Device> {
    vec![microbrute::microbrute(), beatstep::beatstep()]
}

pub fn known_devices_by_name() -> LinkedHashMap<String, Device> {
    known_devices()
        .into_iter()
        .map(|dev| (dev.name.to_owned(), dev))
        .collect()
}
