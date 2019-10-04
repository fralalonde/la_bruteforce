
pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices::{DeviceError, Result, MidiPort, CLIENT_NAME, DevicePort};
use std::convert::TryFrom;
use std::collections::HashMap;
use crate::devices;
use midir::MidiOutput;
use regex::Regex;
use std::str::FromStr;
use linked_hash_map::LinkedHashMap;
use std::fmt::{Display, Formatter, Error};
use std::fmt;

lazy_static!{
    pub static ref SCHEMAS: &'static LinkedHashMap<String, Device> = &load_schemas();
}

fn load_schemas() -> HashMap<String, Device> {
    let mut map = LinkedHashMap::new();
    let mut dev = parse_schema(include_str!("MicroBrute.yaml")).unwrap();
    map.insert(dev.name, dev);
    map
}

fn parse_schema(body: &str) -> Result<Device> {
    Ok(serde_yaml::from_str(body)?)
}

pub struct Vendor {
    pub name: String,
    pub sysex: Vec<u8>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Device {
    pub vendor: Vendor,
    pub port_prefix: String,
    pub sysex: Sysex,
    pub parameters: LinkedHashMap<String, Parameter>,
}

impl Device {

    pub fn locate(&self) -> Result<DevicePort> {
        let client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
        devices::output_ports(&client)
            .into_iter()
            .find(|port| port.name.starts_with(self.port_prefix))
            .map(|port| Some(devices::DevicePort{
                schema: self.clone(),
                client,
                port: *port
            }))
    }

    pub fn parse_key(&self, param_key: &str) -> Result<ParamKey> {
        static RE: Regex = Regex::new(r#"(?P<name>.+)(:?/(?P<idx>\d+))(?::(?P<mode>.+))"#).unwrap();

        if let Some(cap) = RE.captures(param_key) {
            let param_match = cap.name("name")?.as_str();
            let param = self.parameters.get(param_match)?;

            let index_val = if let Some(idx_match) = cap.name("idx") {usize::from_str(idx_match.as_str())?} else {None};
            let index = match (index_val, param.range) {
                (Some(value), Some(range)) if value >= range.lo && value <= range.hi => Some(value),
                (None, None) => None,
                _ => return Err(Box::new(DeviceError::BadIndexParameter{param_name: param_key.to_string()}))
            };

            let mode = match (cap.name("mode"), &param.modes) {
                (Some(mode_str), Some(modes)) => {
                    if let Some(mode) = modes.get(mode_str.as_str()) {
                        Some(mode.clone())
                    } else {
                        return Err(Box::new(DeviceError::BadModeParameter{param_name: param_key.to_string()}))
                    }
                },
                (None, None) => None,
                _ => return Err(Box::new(DeviceError::BadModeParameter{param_name: param_key.to_string()}))
            };

            ParamKey {
                param: param.clone(),
                index,
                mode,
            }
        } else {
            Err(Box::new(DeviceError::UnknownParam{param_name: param_key.to_string()}))
        }
    }

}

pub struct ParamKey {
    pub name: String,
    pub param: Parameter,
    pub index: Option<usize>,
    pub mode: Option<Mode>,
}

impl Display for ParamKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some(index) = &self.index {
            f.write_fmt("/{}", index)?;
        }
        if let Some(mode) = &self.mode {
            f.write_fmt(":{}", mode)?;
        }
        Ok(())
    }
}

impl ParamKey {
    pub fn bounds(&self, field_name: Option<String>) -> Result<Vec<Bounds>> {
        match (self, field_name) {
            (ParamKey{ mode, .. }, Some(field_name)) => {
                mode.fields.get(field_name)
                    .ok_or(DeviceError::BadField { field_name })
            },
            (ParamKey{ param, .. }, None) => param.bounds?,
            _ => Err(Box::new(DeviceError::NoBounds))
        }
    }

    fn fields(&self) -> Option<LinkedHashMap<String, Vec<Bounds>>> {
        self.mode.map(|mode| mode.fields)
    }

    pub fn parse_value(&self, value: &str) -> Result<Value> {
        match (value.split("=").collect(), self.fields()) {
            [field_name, value] => {
                for b in self.bounds(field_name)? {
                    if let Some(v) = b.convert(value) {
                        return Ok(Value::FieldValue(field_name, v));
                    }
                }
            },
            [value] => {
                for b in self.bounds(None)? {
                    if let Some(v) = b.convert(value) {
                        return Ok(Value::ParamValue(v))
                    }
                }
            },
            _ => {}
        }
        Err(Box::new(DeviceError::ValueOutOfBound {value_name: value.to_string()}))
    }

}

pub enum Value {
    ParamValue(String),
    FieldValue(String, String)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub sysex: Sysex,
    pub range: Option<Range>,
    pub bounds: Option<Vec<Bounds>>,
    pub modes: Option<LinkedHashMap<String, Mode>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Mode {
    pub sysex: Sysex,
    pub fields: Fields,
}

pub type Fields = LinkedHashMap<String, Field>;

pub struct Field {
    pub sysex: Sysex,
    pub bounds: Vec<Bounds>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
//#[serde(tag = "type")] /*maybe we can get away without*/
#[serde(untagged)]
pub enum Bounds {
    /// Name / Value pair
    Values(LinkedHashMap<String, u8>),

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
