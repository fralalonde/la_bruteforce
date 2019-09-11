use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use std::time::Duration;

use crate::devices::{MidiValue, Bounds, DeviceError};
use linked_hash_map::LinkedHashMap;
use std::thread::sleep;
use std::str::FromStr;
use strum::IntoEnumIterator;

const CLIENT_NAME: &str = "LaBruteForce";

pub static MIDI_OUT_CLIENT: MidiOutput = MidiOutput::new(CLIENT_NAME).expect("MIDI client initialization failed");

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

#[derive(Debug)]
pub struct MidiPort {
    pub number: usize,
    pub name: String,
}

pub fn output_ports() -> Vec<MidiPort> {
    (0..MIDI_OUT_CLIENT.port_count())
        .filter_map(|idx| MIDI_OUT_CLIENT.port_name(idx)
            .map(|name| MidiPort {
                name: name.clone(),
                number: *idx,
            }))
        .collect()
}

fn input_ports(midi: &MidiInput) -> LinkedHashMap<String, MidiPort> {
    (0..midi.port_count())
        .filter_map(|idx| midi.port_name(idx).map(|name| (name, idx)).ok())
        .collect()
}

const SYSEX_QUERY_START: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];


pub fn sysex_query_init(port_name: &str) -> Result<SysexQuery> {
    let midi_in = MidiInput::new(CLIENT_NAME)?;
    let in_port = *input_ports(&midi_in)
        .get(port_name)
        .ok_or(DeviceError::NoInputPort{ port_name: port_name.to_owned() })?;
    Ok(SysexQuery(midi_in.connect(in_port, "Query Results",
       |_ts, message, results|
           if message[0] == 0xf0 && message[message.len() - 1] == 0xf7 {
               results.push(message.into_vec());
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

