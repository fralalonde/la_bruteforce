
use crate::devices;
use crate::devices::{DeviceError, DevicePort, CLIENT_NAME};

use serde::{Deserialize, Serialize, Deserializer};
use linked_hash_map::LinkedHashMap;
use midir::MidiOutput;
use std::fmt;
use std::str::FromStr;
use std::fmt::Display;
use snafu::Snafu;
use strum::{IntoEnumIterator, ParseError};
use std::num::ParseIntError;
use std::ops::Deref;
use snafu::ResultExt;

type Result<T> = ::std::result::Result<T, SchemaError>;

#[derive(Debug, Snafu)]
pub enum SchemaError {
    BadNoteSyntax,
    SerdeYamlError {
        source: serde_yaml::Error
    },
    EnumParseError {
        source: strum::ParseError
    },
    IntParseError {
        source: std::num::ParseIntError
    }
}

lazy_static! {
    pub static ref VENDORS: LinkedHashMap<String, Vendor> = load_vendors();
    pub static ref DEVICES: LinkedHashMap<String, (&'static Vendor, &'static Device)> = load_devices();
}

fn load_vendors() -> LinkedHashMap<String, Vendor> {
    let mut map = LinkedHashMap::new();
    let vendor = parse_vendor(include_str!("Realtime.yaml")).expect("Realtime not loaded");
    map.insert(vendor.name.clone(), vendor);
    let vendor = parse_vendor(include_str!("Arturia.yaml")).expect("Arturia not loaded");
    map.insert(vendor.name.clone(), vendor);
    map
}

fn load_devices() -> LinkedHashMap<String, (&'static Vendor, &'static Device)> {
    let mut map = LinkedHashMap::new();
    for v in VENDORS.values() {
        for dev in &v.devices {
            map.insert(dev.name.clone(), (v, dev));
        }
    }
    map
}

pub enum Sysex {
    Default(Option<Vec<u8>>),
    Split{
        update: Option<Vec<u8>>,
        request: Option<Vec<u8>>,
        reply: Option<Vec<u8>>
    }
}

pub enum Operation {
    Expect,
    Accept,
    Take(usize),
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Node {
    name: String,
    sysex: Sysex,
    operation: Operation,
    emit: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct AnyOf {
    #[serde(flatten)]
    node: Node,
    any_of: Vec<Box<Visit>>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct AllOf {
    #[serde(flatten)]
    node: Node,
    all_of: Vec<Box<Visit>>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct OneOf {
    #[serde(flatten)]
    node: Node,
    one_of: Vec<Box<Visit>>,
}


pub trait Context {
    fn is_reply(&self) -> bool;

}

pub trait Visit {
    fn visit(&self, context: &mut Context);
}

impl Visit for Node {
    fn visit(&self, context: &mut Context) {
        
    }
}

//pub enum Operator {
//    OneOf
//    AnyOf
//    SeqOf
//}

fn parse_vendor(body: &str) -> Result<Vendor> {
    Ok(serde_yaml::from_str(body).context(SerdeYamlError)?)
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
    pub range: Range,
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
//#[serde(tag = "type")]
#[serde(untagged)]
pub enum Bounds {
    /// Name / Value pair
    Value(Value),

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

#[derive(Debug, Clone, Copy)]
pub struct MidiNote {
    pub note: u8,
}

impl MidiNote {
    fn with_offset(value: u8, offset: i16) -> Self {
        MidiNote{note: (value as i16 + offset) as u8}
    }
}

impl Deref for MidiNote {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.note
    }
}

impl fmt::Display for MidiNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let oct = (self.note - 12) / 12;
        let n = self.note % 12;
        let mut prev_note = NoteName::C;
        for i in NoteName::iter() {
            if i as u8 == n {
                let z: &'static str = i.into();
                f.write_fmt(format_args!("{}{}", z, oct))?;
                break;
            } else if i as u8 > n {
                let z: &'static str = prev_note.into();
                f.write_fmt(format_args!("{}#{}", z, oct))?;
                break;
            } else {
                prev_note = i;
            }
        }
        Ok(())
    }
}

#[derive(Debug, EnumString, IntoStaticStr, EnumIter, AsRefStr, Clone, Copy)]
pub enum NoteName {
    C = 0,
    D = 2,
    E = 4,
    F = 5,
    G = 7,
    A = 9,
    B = 11,
}

impl FromStr for MidiNote {
    type Err = SchemaError;

    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        let mut chars = s.chars();
        let mut item = chars.next();
        if let Some(n) = item {
            let mut note = NoteName::from_str(&n.to_string()).context(EnumParseError)? as u8;
            item = chars.next();
            if let Some(sharp) = item {
                if sharp == '#' {
                    note = note + 1;
                    item = chars.next();
                }
            }
            let octave = match item {
                Some(oct) => u8::from_str(&oct.to_string()).context(IntParseError)?,
                None => 0,
            };
            // C0 starts at 12
            Ok(MidiNote {
                note: octave * 12 + note + 12,
            })
        } else {
            Err(SchemaError::BadNoteSyntax)
        }
    }
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
