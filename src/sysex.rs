use midi::MidiValue;
use std::cmp::Eq;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use midir::{MidiInput, MidiOutput};

use linked_hash_map::LinkedHashMap;
use midir::{MidiOutputConnection, MidiOutput};
use std::time::Duration;

pub type Result<T> = ::std::result::Result<T, Box<::std::error::Error>>;

pub type MidiPort = usize;

pub fn lookup_out_port(midi_out: &MidiOutput, name: &str) -> Option<MidiPort> {
    for i in 0..midi_out.port_count() {
        if midi_out.port_name(i).unwrap().starts_with(name) {
            return Some(i as MidiPort);
        }
    }
    None
}

pub fn lookup_in_port(midi_in: &MidiInput, name: &str) -> Option<MidiPort> {
    for i in 0..midi_in.port_count() {
        if midi_in.port_name(i).unwrap().starts_with(name) {
            return Some(i as MidiPort);
        }
    }
    None
}

//pub fn enum_out_port(midi_out: &MidiOutput) -> HashMap<MidiPort, String> {
//    (0..midi_out.port_count())
//        .filter_map(|port_num| (port_num, midi_out.port_name(port_num)
//            .unwrap_or_else(|err| {
//                eprintln!("Could not get port name: {}", err);
//                None
//            })))
//        .collect()
//}
//
//pub fn enum_in_port(midi_in: &MidiInput) -> Vec<(MidiPort, String)> {
//    (0..midi_in.port_count())
//        .map(|port_num| (port_num, midi_in.port_name(port_num).unwrap()))
//        .collect()
//}

enum DeviceEvent {
    CONNECTED(DeviceCtl),
    DISCONNECTED(DeviceCtl)
}

struct SysexConnection {
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

impl SysexConnection {
    pub fn new(midi_connection: MidiOutputConnection) -> Result<Self> {
        Ok(SysexConnection {
            midi_connection,
            sysex_counter: 0,
        })
    }

    fn init_receiver<F: Fn(u8, MidiValue)>(&mut self, port_name: &str, device_id: u8, callback: F) -> RxHandle {
        let midi_in = MidiInput::new("La_BruteForce In")?;
        let in_port = lookup_in_port(&midi_in, port_name).ok_or("Could not open RX midi port to {}")?;
        let conn_in = midi_in.connect(in_port, "Sysex Query Results",
          |ts, message, state_map| {
              let len = message.len();
              // is sysex message?
              if message[0] == 0xf0 && message[len - 1] == 0xf7 {
                  if is_device_sysex(message, device_id) {
                      let param_id = message[8];
                      let value = message[9] as MidiValue;
                      callback(param_id, value);
                  }
              }
          },
          (),
        )?;

        /// TODO make this a handle
        conn_in.close().1
    }

    fn query_value(&mut self, param: u8) -> Result<()> {
        self.conn_out.send(&SYSEX_QUERY_START)?;
        self.conn_out.send(&sysex_query_msg(sysex_count, *param_id))?;
        self.sysex_count += 1;
    }

    fn send_value(&mut self, param: u8, value: MidiValue) -> Result<Option<MidiValue>> {
        self.conn_out.send(&sysex_update_msg(self.sysex_counter, param, value))?;
        self.sysex_counter += 1;
        Ok(self.state.insert(param, value))
    }
}

fn sysex_query_msg(counter: usize, param: u8) -> [u8; 10] {
    [ 0xf0, 0x00, 0x20, 0x6b, 0x05, 0x01, counter as u8, 0x00, param, 0xf7 ]
}

fn sysex_update_msg(counter: usize, param: u8, value: MidiValue) -> [u8; 11] {
    [ 0xf0, 0x00, 0x20, 0x6b, 0x05, 0x01, counter as u8, 0x01, param as u8, value, 0xf7 ]
}
