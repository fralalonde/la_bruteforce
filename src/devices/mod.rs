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

pub fn sysex_query_init<R, INIT, ITEM>(
    port_name: &str,
    init_fn: INIT,
    item_fn: ITEM,
) -> Result<SysexQuery<R>>
where
    R: Send,
    INIT: FnOnce() -> R,
    ITEM: FnMut(u64, &[u8], &mut R) + Send + 'static,
{
    let midi_in = MidiInput::new(CLIENT_NAME)?;
    let in_port = input_port(&midi_in, port_name)
        .ok_or(DeviceError::NoInputPort { port_name: port_name.to_string() })?;
    Ok(SysexQuery(midi_in.connect(
        in_port.number,
        "Query Results",
        item_fn,
        init_fn(),
    )?))
}

pub struct SysexQuery<T: 'static>(MidiInputConnection<T>);

impl<T> SysexQuery<T> {
    pub fn close_wait(self, wait_millis: u64) -> T {
        sleep(Duration::from_millis(wait_millis));
        self.0.close().1
    }
}

pub type MidiValue = u8;

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
    fn query(&mut self, params: &[String]) -> Result<Vec<(String, String)>>;
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
    UnknownParameterCode {
        code: u32,
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
    WrongDeviceId {
        id: Vec<u8>,
    },
}

fn is_sysex<R, F>(next_filter: F) -> F
    where F: FnMut(u64, &[u8], &mut R) + Send + 'static
{
    |ts, msg, state| {
        let len = msg.length();
        if len > 0
            && msg[0] == 0xf0
            && msg[len -1 ] {
            next_filter(&msg[1..len - 1])
        }
    }
}