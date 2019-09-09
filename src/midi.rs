use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use std::time::Duration;

use crate::devices::MidiValue;
use crate::devices::{DeviceDescriptor, SysexParamId};
use linked_hash_map::LinkedHashMap;
use std::thread::sleep;
use crate::sysex;

pub const CLIENT_NAME: &str = "LaBruteForce";

pub type MidiPort = usize;

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

pub fn output_ports(midi: &MidiOutput) -> LinkedHashMap<String, MidiPort> {
    (0..midi.port_count())
        .filter_map(|idx| midi.port_name(idx).map(|name| (name, idx)).ok())
        .collect()
}

pub fn input_ports(midi: &MidiInput) -> LinkedHashMap<String, MidiPort> {
    (0..midi.port_count())
        .filter_map(|idx| midi.port_name(idx).map(|name| (name, idx)).ok())
        .collect()
}

pub struct SysexConnection {
    device: DeviceDescriptor,
    midi_connection: MidiOutputConnection,
    sysex_counter: usize,
}

const SYSEX_QUERY_START: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];

fn is_device_sysex(message: &[u8], device_code: u8) -> bool {
    message[1] == 0x00 && // Arturia 1
        message[2] == 0x20 && // Arturia 2
        message[3] == 0x6b && // Arturia 3
        message[4] == device_code &&
        message[5] == 0x01 &&
        message[7] == 0x01
}

pub struct RxHandle(MidiInputConnection<LinkedHashMap<SysexParamId, MidiValue>>);

impl RxHandle {
    pub fn close(self, wait_millis: u64) -> LinkedHashMap<SysexParamId, MidiValue> {
        sleep(Duration::from_millis(wait_millis));
        self.0.close().1
    }
}

impl SysexConnection {
    pub fn new(midi_connection: MidiOutputConnection, device: DeviceDescriptor) -> Self {
        SysexConnection {
            device,
            midi_connection,
            sysex_counter: 0,
        }
    }

    pub fn init_receiver(&mut self, port_name: &str, device: &DeviceDescriptor) -> Result<RxHandle> {
        let midi_in = MidiInput::new(CLIENT_NAME)?;
        let in_port = *input_ports(&midi_in)
            .get(port_name)
            // TODO snafu error
            .ok_or(format!(
                "Could not open input midi port for '{}'",
                &device.port_name_prefix
            ))?;
        let sysex_out_id = device.sysex_out_id;
        let conn_in = midi_in.connect(
            in_port,
            "Sysex Query Results",
            move |_ts, message, received_values| {
                if message[0] == 0xf0 && message[message.len() - 1] == 0xf7 {
                    if is_device_sysex(message, sysex_out_id) {
                        let param_id = message[8];
                        let value = message[9] as MidiValue;
                        received_values.insert(param_id, value);
                    }
                }
            },
            LinkedHashMap::new(),
        )?;

        // TODO make this a handle
        Ok(RxHandle(conn_in))
    }

    pub fn query_value(&mut self, param_id: SysexParamId) -> Result<()> {
        self.midi_connection.send(&SYSEX_QUERY_START)?;
        self.midi_connection
            .send(&sysex_query_msg(self.sysex_counter, param_id))?;
        self.sysex_counter += 1;
        Ok(())
    }

    pub fn query_general_information(&mut self) -> Result<()> {
        self.midi_connection.send(unsafe {sysex::Message::default().as_slice()})?;
        self.sysex_counter += 1;
        Ok(())
    }

    pub fn send_value(&mut self, param: SysexParamId, value: MidiValue) -> Result<()> {
        self.midi_connection
            .send(&sysex_update_msg(self.sysex_counter, param, value))?;
        self.sysex_counter += 1;
        Ok(())
    }
}

fn sysex_query_msg(counter: usize, param: SysexParamId) -> [u8; 10] {
    [
        0xf0,
        0x00,
        0x20,
        0x6b,
        0x05,
        0x01,
        counter as u8,
        0x00,
        param,
        0xf7,
    ]
}

fn sysex_update_msg(counter: usize, param: SysexParamId, value: MidiValue) -> [u8; 11] {
    [
        0xf0,
        0x00,
        0x20,
        0x6b,
        0x05,
        0x01,
        counter as u8,
        0x01,
        param as u8,
        value,
        0xf7,
    ]
}
