use midir::MidiOutput;
use midir::{MidiInput, MidiInputConnection};

use std::time::Duration;

use std::thread::sleep;

use crate::error::DeviceError;
use crate::Result;

pub const CLIENT_NAME: &str = "LaBruteForce";

#[derive(Debug, Clone)]
pub struct MidiPort {
    pub number: usize,
    pub name: String,
}

pub type MidiValue = u8;

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

pub fn decode(port_name: &str, filter: FilterFn) -> Result<()>
{
    let midi_in = MidiInput::new(CLIENT_NAME)?;
    let in_port = input_port(&midi_in, port_name)
        .ok_or(DeviceError::NoInputPort { port_name: port_name.to_string() })?;
    Ok(SysexQuery(midi_in.connect(
        in_port.number,
        "Query Results",
        |ts, msg: &[u8], _state| filter.apply(ts, msg),
        (),
    )?))
}

pub struct SysexQuery<T: 'static>(MidiInputConnection<T>);

impl<T> SysexQuery<T> {
    pub fn close_wait(self, wait_millis: u64) -> T {
        sleep(Duration::from_millis(wait_millis));
        self.0.close().1
    }
}

pub fn sysex(next: FilterFn) -> FilterFn {
    FilterFn::new(|ts, msg: &[u8]| {
        let len = msg.len();
        if len > 0
            && msg[0] == 0xf0
            && msg[len - 1] == 0xf7 {
            next(ts, &msg[1..len - 1])
        }
    })
}

pub fn header(bytes: &'static [u8], next: FilterFn) -> FilterFn {
    FilterFn::new(move |ts, msg: &[u8]| {
        let len = msg.len();
        if msg.starts_with(bytes) {
            next(ts, &msg[bytes.len()..len - 1])
        }
    })
}

pub struct FilterFn (Box<dyn Fn(u64, &[u8]) + Send + 'static>);

impl FilterFn {
    pub fn new<F: Fn(u64, &[u8])>(fun: F) -> Self {
        FilterFn(Box::new(fun))
    }
}

//#[derive(Default)]
//struct FilterChain<'a> {
//    chain: Vec<FilterFn<'a>>
//}
//
//impl <'a> FilterChain<'a> {
//    pub fn new(filter: FilterFn<'a>) -> Self {
//        FilterChain { chain: vec![filter] }
//    }
//
//    pub fn add(&mut self, filter: FilterFn<'a>) {
//        self.chain.push(filter)
//    }
//
//    fn apply(&self, ts: u64, mut msg: &'a [u8], state: &mut Vec<Vec<u8>>) {
//        for f in &self.chain {
//            match f.0(ts, msg) {
//                Some(m) => msg = m,
//                None => break
//            }
//        }
//        if !msg.is_empty() {
//            state.push(msg.to_vec())
//        }
//    }
//}
