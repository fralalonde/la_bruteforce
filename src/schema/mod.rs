
pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices::{DeviceError, Result};
use std::convert::TryFrom;
use std::collections::BTreeMap;

//lazy_static!{
//    static ref
//}

#[derive(Debug, EnumString, IntoStaticStr, EnumIter, Display)]
pub enum DeviceType {
    MicroBrute,
    BeatStep,
}

//impl From<DeviceType> for Device {
//    fn from(dev: DeviceType) -> Self {
//
//    }
//}

impl TryFrom<&str> for Device {
    type Error = Box<dyn ::std::error::Error>;

    fn try_from(name: &str) -> Result<Device> {
        match name {
            "MicroBrute" => parse(include_str!("MicroBrute.yaml")),
            _ => Err(Box::new(DeviceError::UnknownDevice {
                device_name: name.to_string(),
            })),
        }
    }
}

fn parse(body: &str) -> Result<Device> {
    Ok(serde_yaml::from_str(body)?)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    vendor: String,
    port_prefix: String,
    sysex: Sysex,
    parameters: BTreeMap<String, Parameter>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    sysex: Sysex,
    index: Option<Range>,
    bounds: Option<Vec<Bounds>>,
    modes: Option<BTreeMap<String, Mode>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    sysex: Sysex,
    fields: Fields,
}

pub type Fields = BTreeMap<String, Vec<Bounds>>;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Bounds {
    /// Name / Value pair
    Values(BTreeMap<String, u8>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(Range),

    /// Sequence of notes with offset from std MIDI note value
    NoteSeq(NoteSeq),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Range {
    lo: u8,
    hi: u8,
    sysex_offset: u8,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct NoteSeq {
    max_len: u8,
    sysex_offset: u8,
}

#[cfg(test)]
mod test {
    use crate::schema::{parse, Device};

    #[test]
    fn test_parse() {
        let z: Device = parse(
            r"
name: MicroBrute
vendor: Arturia
port_prefix: MicroBrute
sysex:
- 0x05
parameters:
  Seq:
    index:
      lo: 1
      hi: 8
      sysex_offset: 1
    sysex:
    - 0x04
    - 0x3a
    bounds:
    - type: NoteSeq
      max_len: 64
      sysex_offset: 24
  StepOn:
    sysex:
    - 0x01
    - 0x3a
    bounds:
    - type: Values
      Gate: 0x01
      Key: 0x02
  MidiRxChan:
    sysex:
    - 0x01
    - 0x3a
    bounds:
    - type: Range
      lo: 1
      hi: 16
      sysex_offset: 1
    - type: Values
      All: 0x10
",
        )
        .unwrap();
        dbg!(z);
    }
}
