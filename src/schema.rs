pub type Sysex = Vec<u8>;

use serde::{Deserialize, Serialize};

use crate::devices;
use crate::device::{DeviceError, DevicePort, MidiNote, Result, CLIENT_NAME};
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
    pub parameters: LinkedHashMap<String, Parameter>,
}

impl Device {
    pub fn locate(&self) -> Result<DevicePort> {
        let client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
        Ok(device::output_ports(&client)
            .into_iter()
            .find(|port| port.name.starts_with(&self.port_prefix))
            .map(|port| device::DevicePort {
                schema: self.clone(),
                client,
                port,
            })
            .ok_or(DeviceError::NoConnectedDevice {
                device_name: self.name.clone(),
            })?)
    }

    pub fn parse_key(&self, param_key: &str) -> Result<QueryKey> {
        let seq_parts: Vec<&str> = param_key.split("/").collect();
        let name: &str = seq_parts.get(0).ok_or("Empty param key")?;
        let mut mode_parts: Vec<&str> = param_key.split(":").collect();
        let (index, mode) =  match (seq_parts.len(), mode_parts.len()) {
            (1, 1) => (None, None),
            (2, 1) => (seq_parts.get(1), None),
            (1, 2) => (None, mode_parts.get(1)),
            (2, 2) => {
                // i.e. "Seq/3:Mode" : re-split "3" from "Mode"
                mode_parts = seq_parts.get(1).unwrap().split(":").collect();
                (mode_parts.get(0), mode_parts.get(1))
            },
            _ => Err(DeviceError::UnknownParameter {
                param_name: param_key.to_string(),
            })?
        };
        let param = self
            .parameters
            .get(name)
            .ok_or(DeviceError::UnknownParameter {
                param_name: param_key.to_string(),
            })?;

        let index_val = if let Some(idx_match) = index {
            Some(usize::from_str(*idx_match)?)
        } else {
            None
        };
        let index = match (index_val, param.range) {
            (Some(value), Some(range)) if value >= range.lo && value <= range.hi => Some(value),
            (None, None) => None,
            _ => {
                return Err(Box::new(DeviceError::BadIndexParameter {
                    param_name: param_key.to_string(),
                }))
            }
        };

        let mode = match (mode, &param.modes) {
            (Some(mode_str), Some(modes)) => {
                if let Some(mode) = modes.get(*mode_str) {
                    Some(mode.clone())
                } else {
                    return Err(Box::new(DeviceError::BadModeParameter {
                        param_name: param_key.to_string(),
                    }));
                }
            }
            (None, None) => None,
            _ => {
                return Err(Box::new(DeviceError::BadModeParameter {
                    param_name: param_key.to_string(),
                }))
            }
        };

        Ok(QueryKey {
            name: name.to_string(),
            param: param.clone(),
            index,
            mode,
        })
    }

    pub fn parse_msg_key(&self, msg: &[u8]) -> Result<QueryKey> {
        let pcode = &msg[2..3];
        for p in self.parameters {
            if p.1.sysex.eq(pcode) {
                return Ok(QueryKey {
                    name: p.0.clone(),
                    param: p.1,
                    index: if p.1.range.is_some() { Some(msg[4] as usize) } else { None },
                    mode: p.1.find_mode(msg)
                })
            }
        }
        Err(DeviceError::UnknownParameterCode { code: hex::encode(pcode) })?
    }
}




impl QueryKey {
    pub fn bounds(&self, field_name: Option<&str>) -> Result<Vec<Bounds>> {
        match (self, field_name) {
            (
                QueryKey {
                    mode: Some(mode), ..
                },
                Some(field_name),
            ) => Ok(mode
                .fields
                .get(field_name)
                .ok_or(DeviceError::BadField {
                    field_name: field_name.to_string(),
                })?
                .bounds
                .clone()),
            (_, None) => Ok(self.param.bounds.clone().ok_or(DeviceError::BadSchema {
                field_name: self.name.clone(),
            })?),
            _ => Err(Box::new(DeviceError::NoBounds)),
        }
    }

    fn fields(&self) -> Option<LinkedHashMap<String, Field>> {
        self.mode.clone().map(|mode| mode.fields)
    }

    pub fn parse_value(&self, value: &str) -> Result<Value> {
        let parts: Vec<&str> = value.split("=").collect();
        match (parts.as_slice(), &self.fields()) {
            ([field_name, value], Some(fields)) => {
                for b in self.bounds(Some(field_name))? {
                    if let Ok(v) = b.convert(value) {
                        return Ok(Value::FieldValue(field_name.to_string(), v));
                    }
                }
            }
            ([value], None) => {
                for b in self.bounds(None)? {
                    if let Ok(v) = b.convert(value) {
                        return Ok(Value::ParamValue(v));
                    }
                }
            }
            _ => {}
        }
        Err(Box::new(DeviceError::ValueOutOfBound {
            value_name: value.to_string(),
        }))
    }
}

pub enum Value {
    ParamValue(Vec<u8>),
    FieldValue(String, Vec<u8>),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub sysex: Sysex,
    pub range: Option<Range>,
    pub bounds: Option<Vec<Bounds>>,
    pub modes: Option<LinkedHashMap<String, Mode>>,
}

impl Parameter {
    pub fn find_mode(&self, msg: &[u8]) -> Result<Option<Mode>> {
        if let Some(modes) = &self.modes {
            for m in modes {
                if m.1.sysex.eq(&msg[6..7]) {
                    return Ok(Some(m.1.clone()))
                }
            }
            Err(DeviceError::UnknownParameterCode { code: hex::encode(pcode) })?
        }
        None
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Mode {
    pub sysex: Sysex,
    pub fields: Fields,
}

impl Mode {
    pub fn find_field(&self, msg: &[u8]) -> Result<Field> {
        let mcode = &msg[8..10];
        for f in &self.fields {
            if m.1.sysex.eq(mcode) {
                return Ok(m.1.clone())
            }
        }
        Err(DeviceError::UnknownModeCode { code: hex::encode(pcode) })?
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for z in &self.fields {
            f.write_fmt(format_args!("{}", z.0))?;
        }
        Ok(f.write_str("}")?)
    }
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
#[serde(internal)]
pub enum Bounds {
    /// Name / Value pair
    Values(LinkedHashMap<String, u8>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(Range),

    /// Sequence of notes with offset from std MIDI note value
    MidiNotes(NoteSeq),
}

impl Bounds {
    pub fn convert(&self, value: &str) -> Result<Vec<u8>> {
        match self {
            Bounds::Values(values) => Ok(vec![*values.get(value).ok_or_else(|| {
                DeviceError::UnknownValue {
                    value_name: value.to_owned(),
                }
            })?]),
            Bounds::Range(range) => {
                let val = usize::from_str(value)?;
                if val >= range.lo && val <= range.hi {
                    Ok(vec![if let Some(offset) = range.offset {
                        (val - offset) as u8
                    } else {
                        val as u8
                    }])
                } else {
                    Err(Box::new(DeviceError::ValueOutOfBound {
                        value_name: value.to_owned(),
                    }))
                }
            }
            Bounds::NoteSeq(noteseq) => {
                let offset = noteseq.offset.unwrap_or(0);
                let mut notes = Vec::with_capacity(noteseq.max_len);
                for v in value.split(",") {
                    notes.push((MidiNote::from_str(v)?.note as i8 + offset) as u8)
                }
                Ok(notes)
            }
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Range {
    pub lo: usize,
    pub hi: usize,
    pub offset: Option<usize>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct NoteSeq {
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
