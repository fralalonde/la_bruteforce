use std::iter::Iterator;

use midir::{MidiInput, MidiInputConnection};
use midir::{MidiOutput, MidiOutputConnection};

//mod beatstep;
//mod microbrute;

use snafu::Snafu;

use std::time::Duration;

use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::thread::sleep;

use crate::schema::ParamKey;
use crate::{devices, schema};
use linked_hash_map::LinkedHashMap;
use std::error::Error;
use strum::IntoEnumIterator;

pub const CLIENT_NAME: &str = "LaBruteForce";

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

pub type MidiValue = u8;

static ARTURIA: &[u8] = &[0x00, 0x20, 0x6b];
static REALTIME: u8 = 0x7e;
static IDENTITY_REPLY: &[u8] = &[REALTIME, 0x01, 0x06, 0x02];

pub struct MidiNote {
    pub note: u8,
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

pub struct DevicePort {
    pub schema: schema::Device,
    pub client: MidiOutput,
    pub port: MidiPort,
}

impl DevicePort {
    pub fn connect(self) -> Result<devices::Device> {
        let connection = self.client.connect(self.port.number, &self.port.name)?;
        let mut device = Device {
            schema: self.schema,
            port: self.port,
            connection,
            msg_id: 0,
        };
        device.identify()?;
        Ok(device)
    }
}

pub fn output_ports(midi_client: &MidiOutput) -> Vec<MidiPort> {
    let mut v = vec![];
    for number in 0..midi_client.port_count() {
        let name = midi_client.port_name(number).unwrap();
        v.push(MidiPort { name, number })
    }
    v
}

fn matching_input_port(midi: &MidiInput, out_port: &str) -> Option<MidiPort> {
    (0..midi.port_count())
        .map(|number| MidiPort {
            name: midi.port_name(number).unwrap(),
            number,
        })
        .find(|port| port.name.eq(out_port))
}

pub struct SysexReceiver(MidiInputConnection<LinkedHashMap<String, Vec<String>>>);

impl SysexReceiver {
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

pub struct Device {
    pub schema: schema::Device,
    pub port: MidiPort,
    connection: MidiOutputConnection,
    msg_id: usize,
}

impl Device {
    pub fn identify(&mut self) -> Result<()> {
        static ID_KEY: &str = "ID";

        let header = [
            self.schema.vendor.sysex.as_slice(),
            self.schema.sysex.as_slice(),
        ]
        .concat();
        let sysex_replies = self.sysex_receiver(IDENTITY_REPLY, move |message, result| {
            if message.starts_with(&header) {
                // TODO could grab firmware version, etc. for return
                let _ = result.insert(ID_KEY.to_string(), vec![]);
            } else {
                eprintln!("received spurious sysex {}, filtering on {}", hex::encode(message), hex::encode(&header));
            }
        })?;
        self.connection
            .send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])?;
        sysex_replies
            .close_wait(500)
            .iter()
            .next()
            .ok_or(DeviceError::NoIdentificationReply)?;
        self.msg_id += 1;
        Ok(())
    }

    pub fn sysex_receiver<D>(&self, match_header: &'static [u8], decode: D) -> Result<SysexReceiver>
    where
        D: Fn(&[u8], &mut LinkedHashMap<String, Vec<String>>) + Send + 'static,
    {
        let midi_in = MidiInput::new(CLIENT_NAME)?;
        if let Some(in_port) = matching_input_port(&midi_in, &self.port.name) {
            Ok(SysexReceiver(midi_in.connect(
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
                port_name: self.port.name.clone(),
            }))
        }
    }

    pub fn query(&mut self, params: &ParamKey) -> Result<Vec<String>> {
        let header = [
            self.schema.vendor.sysex.as_slice(),
            self.schema.sysex.as_slice(),
        ]
        .concat();

        let sysex_replies = self.sysex_receiver(header, |message, result| {

        })?;
        match param.index() {
            Some(idx) => {
                //0x01 MSGID(u8) 0x03,0x3b(SEQ) SEQ_IDX(u8 0 - 7) 0x00 SEQ_OFFSET(u8) SEQ_LEN(0x20)
                self.midi_connection.send(&sysex(
                    MICROBRUTE,
                    &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x00, 0x20]],
                ))?;
                self.msg_id += 1;
                self.midi_connection.send(&sysex(
                    MICROBRUTE,
                    &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x20, 0x20]],
                ))?;
                self.msg_id += 1;
            }
            None => {
                self.midi_connection.send(&sysex(
                    MICROBRUTE,
                    &[&[0x01, self.msg_id as u8], query_code],
                ))?;
                self.msg_id += 1;
            }
        }
        Ok(sysex_replies.close_wait(500))
    }

    pub fn update(&mut self, param: &ParamKey, value_ids: &[String]) -> Result<()> {
        // convert values by mode?>field?>bounds

        // check that all fields filled out

        // send mode & field updates

        Ok(())
    }
}

fn decode(schema: schema::Device, msg: &[u8], result_map: &mut LinkedHashMap<String, Vec<String>>) {
    let param = schema.parse_msg(msg);
    if let Some(param) = into_param(msg) {
        match param {
            NoteSeq(_idx) => {
                let notes = result_map.entry(param.to_string()).or_insert(vec![]);
                for nval in &msg[7..] {
                    if *nval == 0 {
                        break;
                    }
                    if *nval == REST_NOTE {
                        notes.push("_".to_string());
                    } else if *nval < 24 {
                        notes.push(format!("?{}", *nval));
                    } else {
                        notes.push(MidiNote { note: *nval - 24 }.to_string());
                    }
                }
            }
            param => {
                if let Some(bound) = devices::bound_str(bounds(param), &[msg[4]]) {
                    let _ = result_map.insert(param.to_string(), vec![bound]);
                } else {
                    eprintln!(
                        "param {} unbound value code '{}'",
                        param.to_string(),
                        msg[4]
                    );
                }
            }
        }
    };
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
    BadFormatParameter {
        param_name: String,
    },
    BadIndexParameter {
        param_name: String,
    },
    BadModeParameter {
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
    BadField {
        field_name: String,
    },
    BadSchema {
        field_name: String,
    },
    NoBounds,
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
