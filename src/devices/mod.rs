use std::iter::Iterator;

use midir::MidiOutput;
use midir::{MidiInput, MidiInputConnection};

//mod beatstep;
mod microbrute;

use snafu::Snafu;

use std::time::Duration;

use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::thread::sleep;

use std::error::Error;
use strum::IntoEnumIterator;
use linked_hash_map::LinkedHashMap;

pub const CLIENT_NAME: &str = "LaBruteForce";

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

pub type MidiValue = u8;

static ARTURIA: &[u8] = &[0x00, 0x20, 0x6b];
static REALTIME: u8 = 0x7e;
static IDENTITY_REPLY: &[u8] = &[REALTIME, 0x01, 0x06, 0x02];

pub struct MidiNote {
    note: u8,
}

impl Display for MidiNote {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
enum NoteName {
    C = 0,
    D = 2,
    E = 4,
    F = 5,
    G = 7,
    A = 9,
    B = 11,
}

impl FromStr for MidiNote {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        let mut iter = s.chars();
        let mut item = iter.next();
        if let Some(n) = item {
            let mut note = NoteName::from_str(&n.to_string())? as u8;
            item = iter.next();
            if let Some(sharp) = item {
                if sharp == '#' {
                    note = note + 1;
                    item = iter.next();
                }
            }
            let octave = match item {
                Some(oct) => u8::from_str(&oct.to_string())?,
                None => 0,
            };
            // C0 starts at 12
            return Ok(MidiNote {
                note: octave * 12 + note + 12,
            });
        }
        Err(Box::new(DeviceError::NoteParse {
            note: s.to_string(),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct MidiPort {
    pub number: usize,
    pub name: String,
}

pub fn output_ports(midi_client: &MidiOutput) -> Vec<MidiPort> {
    let mut v = vec![];
    for number in 0..midi_client.port_count() {
        let name = midi_client.port_name(number).unwrap();
        v.push(MidiPort { name, number })
    }
    v
}

fn input_port(midi: &MidiInput, name4: &str) -> Option<MidiPort> {
    for number in 0..midi.port_count() {
        if let Ok(name) = midi.port_name(number) {
            if name4.eq(&name) {
                return Some(MidiPort { name, number });
            }
        }
    }
    None
}

pub fn sysex_query_init<D>(
    port_name: &str,
    match_header: &'static [u8],
    decode: D,
) -> Result<SysexQuery>
where
    D: Fn(&[u8], &mut LinkedHashMap<String, Vec<String>>) + Send + 'static,
{
    let midi_in = MidiInput::new(CLIENT_NAME)?;
    if let Some(in_port) = input_port(&midi_in, port_name) {
        Ok(SysexQuery(midi_in.connect(
            in_port.number,
            "Query Results",
            move |_ts, message, result_map| {
                if message[0] == 0xf0
                    && message[message.len() - 1] == 0xf7
                    && message[1..].starts_with(match_header)
                {
                    let subslice = &message[match_header.len() + 1..message.len() - 1];
                    decode(subslice, result_map);
                }
            },
            LinkedHashMap::new(),
        )?))
    } else {
        Err(Box::new(DeviceError::NoInputPort {
            port_name: port_name.to_string(),
        }))
    }
}

pub struct SysexQuery(MidiInputConnection<LinkedHashMap<String, Vec<String>>>);

impl SysexQuery {
    pub fn close_wait(self, wait_millis: u64) -> LinkedHashMap<String, Vec<String>> {
        sleep(Duration::from_millis(wait_millis));
        self.0.close().1
    }
}

#[derive(Debug, EnumString, IntoStaticStr, EnumIter, Display)]
pub enum DeviceType {
    MicroBrute,
    //    BeatStep,
}

impl DeviceType {
    pub fn descriptor(&self) -> Box<dyn Descriptor> {
        match self {
            DeviceType::MicroBrute => Box::new(microbrute::MicroBruteDescriptor {}),
            //            DeviceType::BeatStep => Box::new(beatstep::BeatStepDescriptor {}),
        }
    }
}

pub trait Descriptor {
    fn globals(&self) -> Vec<String>;
    fn bounds(&self, param: &str) -> Result<Bounds>;
    fn ports(&self) -> Vec<MidiPort>;
    fn connect(&self, midi_client: MidiOutput, port: &MidiPort) -> Result<Box<dyn Device>>;
}

pub trait Parameter {
    //    fn from_sysex(message: &[u8]) -> &Parameter;
    //    fn from_str(name: &str) -> &Parameter;
}

pub trait Device {
    fn query(&mut self, params: &[String]) -> Result<LinkedHashMap<String, Vec<String>>>;
    fn update(&mut self, param: &str, value_ids: &[String]) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum Bounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),

    /// Sequence of notes with offset from std MIDI note value
    NoteSeq(u8),
}

#[derive(Debug, Snafu)]
pub enum DeviceError {
    UnknownDevice {
        device_name: String,
    },
    UnknownParameter {
        param_name: String,
    },
    EmptyParameter,
    UnknownValue {
        value_name: String,
    },
    NoConnectedDevice {
        device_name: String,
    },
    NoOutputPort {
        port_name: String,
    },
    NoInputPort {
        port_name: String,
    },
    InvalidParam {
        device_name: String,
        param_name: String,
    },
    NoValueReceived,
    ValueOutOfBound {
        value_name: String,
    },
    NoIdentificationReply,
    WrongId {
        id: Vec<u8>,
    },
    NoteParse {
        note: String,
    },
    MissingValue {
        param_name: String,
    },
    TooManyValues {
        param_name: String,
    },
    ReadSizeError,
}

pub fn bound_str(bounds: Bounds, vcode: &[u8]) -> Option<String> {
    if let Some(first) = vcode.get(0) {
        match bounds {
            Bounds::Discrete(values) => {
                for v in &values {
                    if v.0 == *first {
                        return Some(v.1.to_string());
                    }
                }
            }
            Bounds::Range(offset, (lo, hi)) => {
                if *first >= lo && *first <= hi {
                    return Some((*first + offset).to_string());
                }
            }
            Bounds::NoteSeq(offset) => {
                return Some(
                    vcode
                        .iter()
                        .map(|note| {
                            MidiNote {
                                note: (*note - offset),
                            }
                            .to_string()
                        })
                        .collect::<Vec<String>>()
                        .join(","),
                );
            }
        }
    }
    None
}

pub fn bound_codes(bounds: Bounds, bound_ids: &[String], reqs: (usize, usize)) -> Result<Vec<u8>> {
    if bound_ids.len() < reqs.0 {
        return Err(Box::new(DeviceError::MissingValue {
            param_name: "param".to_string(),
        }));
    }
    if bound_ids.len() > reqs.1 {
        return Err(Box::new(DeviceError::TooManyValues {
            param_name: "param".to_string(),
        }));
    }
    match bounds {
        Bounds::Discrete(values) => {
            let b_id = bound_ids.get(0).unwrap();
            for v in &values {
                if v.1.eq(b_id) {
                    return Ok(vec![v.0]);
                }
            }
            Err(Box::new(DeviceError::UnknownValue {
                value_name: b_id.to_owned(),
            }))
        }
        Bounds::Range(offset, (lo, hi)) => {
            let b_id = bound_ids.get(0).unwrap();
            let val = u8::from_str(b_id)?;
            if val >= lo && val <= hi {
                Ok(vec![val - offset])
            } else {
                Err(Box::new(DeviceError::ValueOutOfBound {
                    value_name: b_id.to_owned(),
                }))
            }
        }
        Bounds::NoteSeq(offset) => {
            let mut bcode = Vec::with_capacity(bound_ids.len());
            for b_id in bound_ids {
                bcode.push(MidiNote::from_str(b_id)?.note + offset);
            }
            Ok(bcode)
        }
    }
}

fn sysex(vendor: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(64);
    msg.push(0xf0);
    msg.extend_from_slice(vendor);
    for p in parts {
        msg.extend_from_slice(p);
    }
    msg.push(0xf7);
    msg
}
