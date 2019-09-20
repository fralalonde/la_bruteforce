use std::iter::Iterator;
use std::str::FromStr;

use midir::MidiOutput;
use midir::{MidiInput, MidiInputConnection};

//mod beatstep;
mod microbrute;

use std::time::Duration;

use std::thread::sleep;
use crate::Result;
use crate::midi::{MidiPort, MidiValue};


static UNIVERSAL: &[u8] = &[0x00];
static ARTURIA: &[u8] = &[0x00, 0x06, 0x02];

#[derive(Debug, EnumString, IntoStaticStr, EnumIter, Display)]
pub enum DeviceType {
    MicroBrute,
}

impl DeviceType {
    pub fn descriptor(&self) -> Box<dyn Descriptor> {
        Box::new(match self {
            DeviceType::MicroBrute => microbrute::MicroBruteDescriptor {},
        })
    }
}

pub trait Descriptor {
    fn parameters(&self) -> Vec<String>;
    fn bounds(&self, param: &str) -> Result<Bounds>;
    fn ports(&self) -> Vec<MidiPort>;
    fn connect(&self, midi_client: MidiOutput, port: &MidiPort) -> Result<Box<dyn Device>>;
}

pub trait Device {
    fn query(&mut self, params: &[String]) -> Result<()>;
    fn update(&mut self, param: &str, value: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum Bounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),
}

pub fn bound_str(bounds: Bounds, vcode: u8) -> Option<String> {
    match bounds {
        Bounds::Discrete(values) => {
            for v in &values {
                if v.0 == vcode {
                    return Some(v.1.to_string());
                }
            }
        }
        Bounds::Range(offset, (lo, hi)) => {
            if vcode >= lo && vcode <= hi {
                return Some((vcode + offset).to_string());
            }
        }
    }
    None
}

pub fn bound_code(bounds: Bounds, bound_id: &str) -> Option<u8> {
    match bounds {
        Bounds::Discrete(values) => {
            for v in &values {
                if v.1.eq(bound_id) {
                    return Some(v.0);
                }
            }
        }
        Bounds::Range(offset, (lo, hi)) => {
            if let Ok(val) = u8::from_str(bound_id) {
                if val >= lo && val <= hi {
                    return Some(val - offset);
                }
            }
        }
    }
    None
}

