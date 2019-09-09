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

#[derive(Debug, Clone)]
pub struct Device {
    usb_vendor_id: u32,
    usb_product_id: u32,
    pub sysex_out_id: u8,
    pub sysex_tx_id: u8,
    pub port_name: &'static str,
    pub name: &'static str,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub sysex_rx_id: SysexParamId,
    pub sysex_tx_id: SysexParamId,
    pub name: &'static str,
    pub bounds: ParameterBounds,
}

#[derive(Debug, Clone)]
pub enum ParameterBounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),
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
