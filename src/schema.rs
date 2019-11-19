use crate::devices;
use crate::devices::{DeviceError, DevicePort, CLIENT_NAME};

use serde::{Deserialize, Serialize};
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
    let node = parse_vendor(include_str!("Realtime.yaml")).expect("Realtime not loaded");
    if let Node::Vendor(vendor) = node {
        map.insert(vendor.vendor.clone(), vendor);
    }
    let node = parse_vendor(include_str!("Arturia.yaml")).expect("Arturia not loaded");
    if let Node::Vendor(vendor) = node {
        map.insert(vendor.vendor.clone(), vendor);
    }
    map
}

fn load_devices() -> LinkedHashMap<String, (&'static Vendor, &'static Device)> {
    let mut map = LinkedHashMap::new();
    for v in VENDORS.values() {
        for node in &v.nodes {
            if let Node::Device(dev) = node {
                map.insert(dev.device.clone(), (v, dev));
            }
        }
    }
    map
}

fn parse_vendor(body: &str) -> Result<Node> {
    Ok(serde_yaml::from_str(body).context(SerdeYamlError)?)
}

pub enum Form {
    Update,
    Query,
    Reply,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub enum Sysex {
    Single(Vec<u8>),
    Split {
        default: Option<Vec<u8>>,
        reply: Option<Vec<u8>>,
        update: Option<Vec<u8>>,
        query: Option<Vec<u8>>,
    }
}

impl Sysex {
    pub fn slice(&self, form: Form) -> &[u8] {
        match self {
            Sysex::Single(single) => &single,
            Sysex::Split {default, reply, update,  query} => {
                &match form {
                    Form::Query => query,
                    Form::Reply => reply,
                    Form::Update => update,
                }.unwrap_or(default.unwrap())
            },
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
#[serde(untagged)]
pub enum Node {
    Vendor(Vendor),
    Device(Device),

    Control(Control),
    IndexedControl(IndexedControl),

    /// Name / Value pair
    Value(Value),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(Range),

    /// Sequence of notes with offset from std MIDI note value
    MidiNotes(MidiNotes),
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Vendor {
    pub vendor: String,
    pub sysex: Sysex,
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Device {
    pub device: String,
    pub sysex: Sysex,
    pub port_prefix: String,
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Control {
    pub control: String,
    pub sysex: Sysex,
    pub nodes: Vec<Node>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct IndexedControl {
    pub indexed_control: String,
    pub sysex: Sysex,
    pub range: Range,
    pub nodes: Vec<Node>,
}

//#[derive(Debug, PartialEq, Deserialize, Clone)]
//pub struct IndexedModal {
//    pub mode: String,
//    pub sysex: Sysex,
//    pub index: Range,
//    pub nodes: Vec<Node>,
//}
//
//pub type Fields = LinkedHashMap<String, Field>;
//
//#[derive(Debug, PartialEq, Deserialize, Clone)]
//pub struct Field {
//    pub field: String,
//    pub sysex: Sysex,
//    pub nodes: Vec<Node>,
//}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Value {
    pub value: String,
    pub sysex: Sysex,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct Range {
    pub lo: isize,
    pub hi: isize,
    pub offset: Option<isize>,
}

#[derive(Debug, PartialEq, Deserialize, Clone)]
pub struct MidiNotes {
    pub max_notes: usize,
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
    use crate::schema::{parse_vendor, Device, Vendor, Node};

    #[test]
    fn test_parse() {
        let z: Node = parse_vendor(
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
