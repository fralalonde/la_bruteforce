use std::iter::Iterator;

use midir::{MidiInput, MidiInputConnection};
use midir::{MidiOutput, MidiOutputConnection};

use snafu::Snafu;
use crate::parse;

use std::time::Duration;

use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::thread::sleep;

use crate::{devices, schema};
use linked_hash_map::LinkedHashMap;
use std::error::Error;
use strum::IntoEnumIterator;
use crate::schema::MidiNote;
use crate::parse::{Token, SysexReply, AST};
use snafu::ResultExt;

pub const CLIENT_NAME: &str = "LaBruteForce";

type Result<T> = ::std::result::Result<T, DeviceError>;

pub type MidiValue = u8;

static REALTIME: u8 = 0x7e;
static IDENTITY_REPLY: &[u8] = &[REALTIME, 0x01, 0x06, 0x02];

#[derive(Debug, Snafu)]
pub enum DeviceError {
    UnknownDevice {
        device_name: String,
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
    NoValueReceived,
    NoIdentificationReply,
    WrongId {
        id: Vec<u8>,
    },
    NoteParse {
        note: String,
    },
    ReadSizeError,
    OutputConnectError {
        source: midir::ConnectError<MidiOutput>
    },
    InputConnectError {
        source: midir::ConnectError<MidiInput>
    },
    MidiSendError {
        source: midir::SendError
    },
    MidiInitError {
        source: midir::InitError
    },
    ParseError {
        source: parse::ParseError
    }
}

#[derive(Debug, Clone)]
pub struct MidiPort {
    pub number: usize,
    pub name: String,
}

pub struct DevicePort {
    pub vendor: &'static schema::Vendor,
    pub device: &'static schema::Device,
    pub client: MidiOutput,
    pub port: MidiPort,
}

impl DevicePort {
    pub fn connect(self) -> Result<devices::Device> {
        let connection = self.client.connect(self.port.number, &self.port.name).context(OutputConnectError)?;
        let mut device = Device {
            vendor: self.vendor,
            device: self.device,
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

pub struct SysexReceiver(MidiInputConnection<SysexReply>);

impl SysexReceiver {
    pub fn close_wait(self, wait_millis: u64) -> SysexReply {
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
    pub vendor:  &'static schema::Vendor,
    pub device:  &'static schema::Device,
    pub port: MidiPort,
    connection: MidiOutputConnection,
    msg_id: usize,
}

pub fn locate(vendor: &'static schema::Vendor, device: &'static schema::Device, _index: u8) -> Result<DevicePort> {
    // TODO support index for multiple devices of same model
    let client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
    Ok(devices::output_ports(&client)
        .into_iter()
        .find(|port| port.name.starts_with(&device.port_prefix))
        .map(|port| devices::DevicePort {
            vendor,
            device,
            client,
            port,
        })
        .ok_or(DeviceError::NoConnectedDevice {
            device_name: device.name.clone(),
        })?)
}

impl Device {

    pub fn identify(&mut self) -> Result<()> {
        static ID_KEY: &str = "ID";

        let header = [
            self.vendor.sysex.as_slice(),
            self.device.sysex.as_slice(),
        ]
        .concat();
        let sysex_replies = self.sysex_receiver()?;
        self.connection
            .send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7]).context(MidiSendError)?;
        self.msg_id += 1;

        // TODO match vendor & device tokens
        if sysex_replies.close_wait(500).collect().find_map(& |token| if let Token::Vendor(v) = *token {Some(v)} else {None}).is_none() {
            Err(DeviceError::NoIdentificationReply)
        } else {
            Ok(())
        }
    }

    pub fn sysex_receiver(&self) -> Result<SysexReceiver> {
        let midi_in = MidiInput::new(CLIENT_NAME).context(MidiInitError)?;
        if let Some(in_port) = matching_input_port(&midi_in, &self.port.name) {
            Ok(SysexReceiver(midi_in.connect(
                in_port.number,
                "Query Results",
                |_ts, message, reply| {reply.parse(message).map_err(|err| eprintln!("{:?}", err));},
                SysexReply::new(),
            ).context(InputConnectError)?))
        } else {
            Err(DeviceError::NoInputPort {
                port_name: self.port.name.clone(),
            })
        }
    }

    pub fn query(&mut self, root: &AST) -> Result<String> {
        let receiver = self.sysex_receiver()?;
        let messages = root.to_sysex(&mut self.msg_id).context(ParseError)?;
        for msg in messages {
            self.connection.send(&msg).context(MidiSendError)?
        }
        let reply = receiver.close_wait(500);
        Ok(/* TODO print reply AST*/ "".to_owned())
    }

    pub fn update(&mut self, root: &AST) -> Result<()> {
        // convert values by mode?>field?>bounds

        // check that all fields filled out

        // send mode & field updates

        Ok(())
    }
}
