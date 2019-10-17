pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices;
use crate::devices::{DeviceError, DevicePort, MidiNote, Result, CLIENT_NAME};
use linked_hash_map::LinkedHashMap;
use midir::MidiOutput;
use std::fmt;
use std::str::FromStr;

lazy_static! {
    pub static ref SCHEMAS: LinkedHashMap<String, Device> = load_schemas();
}

fn load_schemas() -> LinkedHashMap<String, Device> {
    let mut map = LinkedHashMap::new();
    let dev = parse_schema(include_str!("MicroBrute.yaml")).expect("MicroBrute");
    map.insert(dev.name.clone(), dev);
    let dev = parse_schema(include_str!("BeatStep.yaml")).expect("BeatStep");
    map.insert(dev.name.clone(), dev);
    map
}

fn parse_schema(body: &str) -> Result<Device> {
    Ok(serde_yaml::from_str(body)?)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Vendor {
    pub name: String,
    pub sysex: Vec<u8>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Device {
    pub name: String,
    pub vendor: Vendor,
    pub port_prefix: String,
    pub sysex: Sysex,

    pub controls: Option<Vec<Control>>,
    pub indexed_controls: Option<Vec<Control>>,
    pub indexed_modal_controls: Option<Vec<Control>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Control {
    pub name: String,
    pub sysex: Sysex,
    pub bounds: Option<Vec<Bounds>>,
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Value {
    pub name: String,
    pub sysex: Sysex,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Range {
    pub lo: usize,
    pub hi: usize,
    pub offset: Option<usize>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct MidiNotes {
    pub max_len: usize,
    pub offset: Option<i8>,
}

#[cfg(test)]
mod test {
    use crate::schema::{parse_schema, Device};

    #[test]
    fn test_parse() {
        let z: Device = parse_schema(
r"
name: MicroBrute
vendor: Arturia
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
