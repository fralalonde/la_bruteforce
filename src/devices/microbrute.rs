use crate::devices::Bounds::*;
use crate::devices::{MidiValue, Bounds, Device, Parameter, Descriptor};
use crate::midi::{SysexQuery, MIDI_OUT_CLIENT};
use linked_hash_map::LinkedHashMap;
use midi::Result;
use midir::{MidiInput, MidiOutput, MidiOutputConnection};
use strum::IntoEnumIterator;
use crate::midi::{self, MidiPort};
use crate::devices::DeviceError;
use std::thread::sleep;
use std::str::FromStr;

//            usb_vendor_id: 0x1c75,
//            usb_product_id: 0x0206,
//            sysex_tx_id: 0x06,

const MICROBRUTE_SYSEX_REQUEST: u8 = 0x06;
const MICROBRUTE_SYSEX_REPLY: u8 = 0x05;

#[derive(Debug, EnumString, IntoStaticStr, EnumIter)]
enum MicrobruteParameter {
    KeyNotePriority,
    KeyVelocityResponse,
    MidiRecvChan,
    MidiSendChan,
    LfoKeyRetrig,
    EnvLegatoMode,
    BendRange,
    Gate,
    Sync,
    SeqPlay,
    SeqKeyRetrig,
    SeqNextSeq,
    SeqStep,
}

#[derive(Debug)]
pub struct MicroBruteDescriptor {
}

impl Descriptor for MicroBruteDescriptor {

    fn parameters(&self) -> Vec<Parameter> {
        MicrobruteParameter::iter().map(|p| p.into()).collect()
    }

    fn bounds(&self, param: Parameter) -> Bounds {
        bounds(param)
    }

    fn ports(&self) -> Vec<MidiPort> {
        midi::output_ports().iter()
            .filter(|(pname, idx)| pname.starts_with("MicroBrute"))
            .collect()
    }

    fn connect(&self, port: &MidiPort) -> Result<Box<Device>> {
        Ok(MicroBruteDevice {
            midi_connection: MIDI_OUT_CLIENT.connect(port.number, &port.name)?,
            port_name: port.name.to_owned(),
            sysex_counter: 0,
        })
    }
}

fn bounds(param: Parameter) -> Bounds {
    let p = MicrobruteParameter::from_str(param);
    match p {
        KeyNotePriority => Discrete(vec![(0, "LastNote"), (1, "LowNote"), (2, "HighNote")]),
        KeyVelocityResponse => {
            Discrete(vec![(0, "Logarithmic"), (1, "Exponential"), (2, "Linear")])
        }
        MidiRecvChan => Range(1, (1, 16)),
        MidiSendChan => Range(1, (1, 16)),
        LfoKeyRetrig => Discrete(vec![(0, "Off"), (1, "On")]),
        EnvLegatoMode => Discrete(vec![(0, "Off"), (1, "On")]),
        BendRange => Range(1, (1, 12)),
        Gate => Discrete(vec![(1, "Short"), (2, "Medium"), (3, "Long")]),
        Sync => Discrete(vec![(0, "Auto"), (1, "Internal"), (2, "External")]),
        SeqPlay => Discrete(vec![(0, "Hold"), (1, "NoteOn")]),
        SeqKeyRetrig => Discrete(vec![(0, "Reset"), (1, "Legato"), (2, "None")]),
        SeqNextSeq => Discrete(vec![(0, "End"), (1, "Reset"), (2, "Continue")]),
        SeqStep => Discrete(vec![
            (0x04, "1/4"),
            (0x08, "1/8"),
            (0x10, "1/16"),
            (0x20, "1/32"),
        ]),
        SeqStepOn => Discrete(vec![(0, "Clock"), (1, "Gate")]),
    }
}


#[derive(Debug)]
pub struct MicroBruteDevice {
    midi_connection: MidiOutputConnection,
    port_name: String,
    sysex_counter: usize,
}

impl Device for MicroBruteDevice {
    fn query(&mut self, params: &[Parameter]) -> Result<Vec<(Parameter, MidiValue)>> {
        let mut sysex_replies = midi::sysex_query_init(&self.port_name);
        for param in params {
            let p = MicrobruteParameter::from_str(param)?;
            self.midi_connection.send(&sysex_query_msg(self.sysex_counter, p))?;
            self.sysex_counter += 1;
        }
        Ok(sysex_replies.close_wait(500)
            .map(|msg| (MicrobruteParameter::BendRange, msg[9]))
            .collect()
        )
    }

    fn update(&mut self, param: Parameter, value: &str) -> Result<()> {
        let p = MicrobruteParameter::from_str(param)?;
        let v = match bounds(param) {
            Bounds::Discrete(values) => values.iter()
                .find(|d| d.1.eq(value))
                .ok_or(DeviceError::UnknownValue{})?.0,
            Bounds::Range(offset, (lo, hi)) => u8::from_str(value)?
                .ok_or(DeviceError::UnknownValue{})? - offset
        };
        self.midi_connection
            .send(&sysex_update_msg(self.sysex_counter, p, v))?;
        self.sysex_counter += 1;
        Ok(())
    }
}
//fn is_device_sysex(message: &[u8], device_code: u8) -> bool {
//    message[1] == 0x00 && // Arturia 1
//        message[2] == 0x20 && // Arturia 2
//        message[3] == 0x6b && // Arturia 3
//        message[4] == device_code &&
//        message[5] == 0x01 &&
//        message[7] == 0x01
//}


impl MicroBruteDevice {
    fn sysex_cmd_out(param: MicrobruteParameter) -> (u8, u8) {
        match param {
            KeyNotePriority => (0x0c, 0x0b),
            KeyVelocityResponse => (0x11, 0x10),
            MidiRecvChan => (0x06, 0x05),
            MidiSendChan => (0x08, 0x07),
            LfoKeyRetrig => (0x0f, 0x0e),
            EnvLegatoMode => (0x0d, 0x0c),
            BendRange => (0x2c, 0x2b),
            Gate => (0x36, 0x35),
            Sync => (0x3c, 0x3b),
            SeqPlay => (0x2e, 0x2d),
            SeqKeyRetrig => (0x34, 0x33),
            SeqNextSeq => (0x32, 0x31),
            SeqStep =>(0x38, 0x37),
            SeqStepOn => (0x2a, 0x2b),
        }
    }
//    if is_device_sysex(message, MICROBRUTE_SYSEX_REPLY) {
//        let param_id = message[8];
//        let value = message[9] as MidiValue;
//        received_values.insert(param_id, value);
//    }
}

fn sysex_query_msg(counter: usize, param: u8) -> [u8; 10] {
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

fn sysex_update_msg(counter: usize, param: u8, value: MidiValue) -> [u8; 11] {
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


