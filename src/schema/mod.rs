pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices;
use crate::devices::{DeviceError, DevicePort, MidiNote, Result, CLIENT_NAME};
use linked_hash_map::LinkedHashMap;
use midir::MidiOutput;
use std::fmt;
use std::str::FromStr;

lazy_static! {
    pub static ref VENDORS: LinkedHashMap<String, Vendor> = load_vendors();
    pub static ref DEVICES: LinkedHashMap<String, (&'static Vendor, &'static Device)> = load_devices();
}

fn load_vendors() -> LinkedHashMap<String, Vendor> {
    let mut map = LinkedHashMap::new();
    let vendor = parse_vendor(include_str!("Arturia.yaml")).expect("Arturia not loaded");
    map.insert(vendor.name.clone(), vendor);
    map
}

fn load_devices() -> LinkedHashMap<String, &'static Device> {
    let mut map = LinkedHashMap::new();
    for v in VENDORS.values() {
        for dev in &v.devices {
            map.insert(dev.name.clone(), (v, dev));
        }
    }
    map
}

fn parse_vendor(body: &str) -> Result<Vendor> {
    Ok(serde_yaml::from_str(body)?)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Vendor {
    pub name: String,
    pub sysex: Vec<u8>,
    pub devices: Vec<Device>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Device {
    pub name: String,
    pub vendor: Vendor,
    pub port_prefix: String,
    pub sysex: Sysex,

    pub controls: Option<Vec<Control>>,
    pub indexed_controls: Option<Vec<IndexedControl>>,
//    pub indexed_modal_controls: Option<Vec<Control>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Control {
    pub name: String,
    pub sysex: Sysex,
    pub bounds: Vec<Bounds>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct IndexedControl {
    pub name: String,
    pub sysex: Sysex,
    pub index: Range,
    pub bounds: Vec<Bounds>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct IndexedModal {
    pub name: String,
    pub sysex: Sysex,
    pub index: Range,
    pub modes: Vec<Value>,
    pub fields: Vec<Fields>,
}

pub type Fields = LinkedHashMap<String, Field>;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Field {
    pub name: String,
    pub sysex: Sysex,
    pub bounds: Vec<Bounds>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
//#[serde(tag = "type")] /*maybe we can get away without*/
#[serde(untagged)]
pub enum Bounds {
    /// Name / Value pair
    Values(Vec<Value>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(Range),

    /// Sequence of notes with offset from std MIDI note value
    MidiNotes(MidiNotes),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Value {
    pub name: String,
    pub sysex: u8,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Range {
    pub lo: isize,
    pub hi: isize,
    pub offset: Option<isize>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MidiNotes {
    pub max_len: usize,
    pub offset: Option<i16>,
}

#[cfg(test)]
mod test {
    use crate::schema::{parse_vendor, Device, Vendor};

    #[test]
    fn test_parse() {
        let z: Vendor = parse_vendor(
r"
name: Arturia
sysex: [0x00]
devices:
  - name: MicroBrute
    port_prefix: MicroBrute
    sysex:
    - 0x05
    indexed_controls:
      Sequence:
        range:
          lo: 1
          hi: 8
          offset: 1
        sysex:
        - 0x04
        - 0x3a
        bounds:
        - max_len: 64
          offset: 24
    controls:
      StepOn:
        sysex:
        - 0x01
        - 0x3a
        bounds:
        - Gate: 0x01
          Key: 0x02
      MidiRxChan:
        sysex:
        - 0x01
        - 0x3a
        bounds:
        - lo: 1
          hi: 16
          offset: 1
        - All: 0x10
      SeqStep:
        sysex:
          - 0x01
          - 0x38
        bounds:
          - type: Values
            - name: 1/4
              sysex: 0x04
            - name: 1/8
              sysex: 0x08
            - name: 1/16
              sysex: 0x10
            - name: 1/32
              sysex: 0x20
",
        )
        .unwrap();
        dbg!(z);
    }
}
