use std::iter::Iterator;

use midir::MidiOutput;
use midir::{MidiInput, MidiInputConnection};

//mod beatstep;
mod microbrute;

use snafu::Snafu;

use std::time::Duration;

use std::thread::sleep;

pub const CLIENT_NAME: &str = "LaBruteForce";

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

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

pub fn sysex_query_init(port_name: &str) -> Result<SysexQuery> {
    let midi_in = MidiInput::new(CLIENT_NAME)?;
    let in_port = input_port(&midi_in, port_name).expect("FUCK RUST ERRORS");
    Ok(SysexQuery(midi_in.connect(
        in_port.number,
        "Query Results",
        |_ts, message, results| {
            if message[0] == 0xf0 && message[message.len() - 1] == 0xf7 {
                results.push(message.to_vec());
            }
        },
        Vec::new(),
    )?))
}

pub struct SysexQuery(MidiInputConnection<Vec<Vec<u8>>>);

impl SysexQuery {
    pub fn close_wait(self, wait_millis: u64) -> Vec<Vec<u8>> {
        sleep(Duration::from_millis(wait_millis));
        self.0.close().1
    }
}

pub type MidiValue = u8;
pub type Parameter = &'static str;

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
    fn parameters(&self) -> Vec<Parameter>;
    fn bounds(&self, param: &str) -> Result<Bounds>;
    fn ports(&self) -> Vec<MidiPort>;
    fn connect(&self, midi_client: MidiOutput, port: &MidiPort) -> Result<Box<dyn Device>>;
}

pub trait Device {
    fn query(&mut self, params: &[String]) -> Result<Vec<(Parameter, MidiValue)>>;
    fn update(&mut self, param: &str, value: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum Bounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),
}

#[derive(Debug, Snafu)]
pub enum DeviceError {
    UnknownDevice {
        device_name: String,
    },
    UnknownParameter {
        param_name: String,
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
    NoReply,
    WrongId {
        id: Vec<u8>,
    },
}
