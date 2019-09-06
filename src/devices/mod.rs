use linked_hash_map::LinkedHashMap;
use midir::{MidiInput, MidiOutput};
use std::iter::Iterator;

use crate::devices::ParameterBounds::*;
use crate::midi::MidiPort;

mod beatstep;
mod microbrute;

pub type MidiValue = u8;
pub type SysexParamId = u8;

enum HotplugEvent {
    CONNECTED(&'static Device),
    DISCONNECTED(&'static Device),
}

pub struct Device {
    usb_vendor_id: u32,
    usb_product_id: u32,
    sysex_out_id: u8,
    sysex_cmd_id: u8,
    pub name: &'static str,
    params: Vec<Param>,
}

pub struct Param {
    sysex_out_id: SysexParamId,
    sysex_cmd_id: SysexParamId,
    name: &'static str,
    bounds: ParameterBounds,
}

#[derive(Debug)]
pub enum ParameterBounds {
    Discrete(Vec<(MidiValue, &'static str)>),
    Range(MidiValue, MidiValue),
}

lazy_static! {
    pub static ref DEVICES: Vec<Device> = vec![microbrute::microbrute(), beatstep::beatstep(),];
    pub static ref PORT_NAMES: LinkedHashMap<&'static str, Device> = port_names();
}

fn port_names() -> LinkedHashMap<&'static str, Device> {
    let mut map = LinkedHashMap::new();
    for dev in vec![microbrute::microbrute(), beatstep::beatstep()] {
        map.insert(dev.name, dev);
    }
    map
}

pub fn port_devices(
    ports: &LinkedHashMap<String, MidiPort>,
) -> LinkedHashMap<MidiPort, &'static Device> {
    PORT_NAMES
        .iter()
        .filter_map(|(dname, dev)| {
            ports
                .iter()
                .find(|(pname, idx)| pname.starts_with(dname))
                .map(|(_dname, idx)| (*idx, dev))
        })
        .collect()
}
