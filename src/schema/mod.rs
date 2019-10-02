
pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices::{DeviceError, Result, MidiPort, CLIENT_NAME};
use std::convert::TryFrom;
use std::collections::BTreeMap;
use crate::devices;
use midir::MidiOutput;
use regex::Regex;
use std::str::FromStr;

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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Device {
    pub vendor: String,
    pub port_prefix: String,
    pub sysex: Sysex,
    pub parameters: BTreeMap<String, Parameter>,
}

impl Device {
//    fn parameters(&self) -> Vec<String> {
//        MicrobruteGlobals::iter()
//            .flat_map(|p| {
//                if let Some(max) = p.max_index() {
//                    (1..=max)
//                        .map(|idx| format!("{}/{}", p.as_ref(), idx))
//                        .collect()
//                } else {
//                    vec![p.as_ref().to_string()]
//                }
//            })
//            .collect()
//    }
//
//    fn bounds(&self, param: &str) -> Result<Bounds> {
//        Ok(bounds(MicrobruteGlobals::parse(param)?))
//    }

    fn read_key(&self, param_str: String) -> Result<ParamKey> {
        static RE: Regex = Regex::new(r"(?P<name>.+)(:?/(?P<idx>\d+))(?::(?P<mode>.+))")?;

        if let Some(cap) = RE.captures(&param_str) {
            let param_match = cap.name("name")?.as_str();
            let param = self.parameters.get(param_match)?;

            let index_val = if let Some(idx_match) = cap.name("idx") {usize::from_str(idx_match.as_str())?} else {None};
            let index = match (index_val, param.range) {
                (Some(value), Some(range)) if value >= range.lo && value <= range.hi => Some(value),
                (None, None) => None,
                _ => return Err(Box::new(DeviceError::BadIndexParameter{param_name: param_str.to_string()}))
            };

            let mode = match (cap.name("mode"), &param.modes) {
                (Some(mode_str), Some(modes)) => {
                    if let Some(mode) = modes.get(mode_str.as_str()) {
                        Some(*mode)
                    } else {
                        return Err(Box::new(DeviceError::BadModeParameter{param_name: param_str.to_string()}))
                    }
                },
                (None, None) => None,
                _ => return Err(Box::new(DeviceError::BadModeParameter{param_name: param_str.to_string()}))
            };

            ParamKey {
                param: param.clone(),
                index,
                mode,
            }
        } else {
            Err(Box::new(DeviceError::UnknownParam{param_name: param_str.to_string()}))
        }
    }

}

pub struct ParamKey {
    param: Parameter,
    index: Option<usize>,
    mode: Option<Mode>,
}

impl ParamKey {
    fn read_values(&self, key: &ParamKey, values_str: Vec<String>) -> Result<Vec<u8>> {

    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub sysex: Sysex,
    pub range: Option<Range>,
    pub bounds: Option<Vec<Bounds>>,
    pub modes: Option<BTreeMap<String, Mode>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Mode {
    pub sysex: Sysex,
    pub fields: Fields,
}

pub type Fields = BTreeMap<String, Vec<Bounds>>;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
//#[serde(tag = "type")]
#[serde(untagged)]
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
    pub lo: usize,
    pub hi: usize,
    pub offset: Option<usize>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct NoteSeq {
    pub max_len: u8,
    pub offset: Option<usize>,
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
      - 1/4: 0x04
        1/8: 0x08
        1/16: 0x10
        1/32: 0x20
",
        )
        .unwrap();
        dbg!(z);
    }
}
