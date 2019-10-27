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

pub const CLIENT_NAME: &str = "LaBruteForce";

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

pub type MidiValue = u8;

static REALTIME: u8 = 0x7e;
static IDENTITY_REPLY: &[u8] = &[REALTIME, 0x01, 0x06, 0x02];

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


#[derive(Debug, Clone)]
pub struct MidiPort {
    pub number: usize,
    pub name: String,
}

pub struct DevicePort {
    pub schema: &'static schema::Device,
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
    pub schema:  &'static schema::Device,
    pub port: MidiPort,
    connection: MidiOutputConnection,
    msg_id: usize,
}

pub fn locate(dev: &schema::Device, index: usize) -> Result<DevicePort> {
    let client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
    Ok(devices::output_ports(&client)
        .into_iter()
        .find(|port| port.name.starts_with(&dev.port_prefix))
        .map(|port| devices::DevicePort {
            schema: dev,
            client,
            port,
        })
        .ok_or(DeviceError::NoConnectedDevice {
            device_name: dev.name.clone(),
        })?)
}

impl Device {

    pub fn identify(&mut self) -> Result<()> {
        static ID_KEY: &str = "ID";

        let header = [
            self.schema.vendor.sysex.as_slice(),
            self.schema.sysex.as_slice(),
        ]
        .concat();
        let sysex_replies = self.sysex_receiver()?;
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

    pub fn sysex_receiver<D>(&self) -> Result<SysexReceiver>
    where
        D: Fn(&[u8], &mut SysexReply) + Send + 'static,
    {
        let midi_in = MidiInput::new(CLIENT_NAME)?;
        if let Some(in_port) = matching_input_port(&midi_in, &self.port.name) {
            Ok(SysexReceiver(midi_in.connect(
                in_port.number,
                "Query Results",
                |_ts, message, reply| reply.parse(message),
                SysexReply::new(),
            )?))
        } else {
            Err(Box::new(DeviceError::NoInputPort {
                port_name: self.port.name.clone(),
            }))
        }
    }

    pub fn query(&mut self, root: &AST) -> Result<String> {
        let receiver = self.sysex_receiver()?;
        let messages = root.to_sysex(&mut self.msg_id);
        for msg in messages {
            self.midi_connection.send(&msg)?
        }
        let reply = receiver.close_wait(500);
        Ok(/* TODO print reply AST*/ "".to_owned())
    }

    pub fn update(&mut self, tokens: &[Token]) -> Result<()> {
        // convert values by mode?>field?>bounds

        // check that all fields filled out

        // send mode & field updates

        Ok(())
    }
}

fn sysex(root: &AST) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(64);
    buffer.push(0xf0);
    root.to_sysex(&mut buffer);
    buffer.push(0xf7);
    buffer
}
