use self::MicrobruteParameter::*;
use crate::devices::Bounds::*;
use crate::devices::DeviceError;
use crate::devices::CLIENT_NAME;
use crate::devices::{self, MidiPort};
use crate::devices::{Bounds, Descriptor, Device, MidiValue};

use devices::Result;
use midir::{MidiOutput, MidiOutputConnection};
use std::str::FromStr;
use std::string::ToString;
use strum::IntoEnumIterator;

//            usb_vendor_id: 0x1c75,
//            usb_product_id: 0x0206,

//const MICROBRUTE_SYSEX_REQUEST: u8 = 0x06;
//const MICROBRUTE_SYSEX_REPLY: u8 = 0x05;

#[derive(Debug, AsRefStr, EnumString, IntoStaticStr, EnumIter)]
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
    SeqStepOn,
}

#[derive(Debug)]
pub struct MicroBruteDescriptor {}

impl Descriptor for MicroBruteDescriptor {
    fn parameters(&self) -> Vec<String> {
        MicrobruteParameter::iter()
            .map(|p| p.as_ref().to_owned())
            .collect()
    }

    fn bounds(&self, param: &str) -> Result<Bounds> {
        let param = MicrobruteParameter::from_str(param)?;
        Ok(bounds(&param))
    }

    fn ports(&self) -> Vec<MidiPort> {
        let midi_client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
        devices::output_ports(&midi_client)
            .into_iter()
            .filter_map(|port| {
                if port.name.starts_with("MicroBrute") {
                    Some(port)
                } else {
                    None
                }
            })
            .collect()
    }

    fn connect(&self, midi_client: MidiOutput, port: &MidiPort) -> Result<Box<dyn Device>> {
        let midi_connection = midi_client.connect(port.number, &port.name)?;
        let mut brute = Box::new(MicroBruteDevice {
            midi_connection,
            port_name: port.name.to_owned(),
            sysex_counter: 0,
        });
        brute.identify()?;
        Ok(brute)
    }
}

fn bounds(param: &MicrobruteParameter) -> Bounds {
    match param {
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

fn bound_str(bounds: Bounds, vcode: u8) -> Option<String> {
    match bounds {
        Bounds::Discrete(values) => {
            for v in &values {
                if v.0 == vcode {
                    return Some(v.1.to_string());
                }
            }
        }
        Bounds::Range(offset, (lo, hi)) => {
            if vcode >= lo && vcode <= hi {
                return Some((vcode + offset).to_string());
            }
        }
    }
    None
}

fn bound_code(bounds: Bounds, bound_id: &str) -> Option<u8> {
    match bounds {
        Bounds::Discrete(values) => {
            for v in &values {
                if v.1.eq(bound_id) {
                    return Some(v.0);
                }
            }
        }
        Bounds::Range(offset, (lo, hi)) => {
            if let Ok(val) = u8::from_str(bound_id) {
                if val >= lo && val <= hi {
                    return Some(val - offset);
                }
            }
        }
    }
    None
}

pub struct MicroBruteDevice {
    midi_connection: MidiOutputConnection,
    port_name: String,
    sysex_counter: usize,
}

impl MicroBruteDevice {
    // TODO return device version / id string
    fn identify(&mut self) -> Result<()> {
        let sysex_replies = devices::sysex_query_init(
            &self.port_name,
            || Some(DeviceError::NoReply),
            |_ts, msg, payload| {
                if msg.starts_with(&[
                    0xf0, 0x7e, 0x01, /* arturia1 */ 0x06, /* arturia2 */ 0x02, 0x00,
                    0x20, 0x6b, 0x04, 0x00, 0x02, 0x01, /* major version */ 0x01,
                    0x00,
                    // remaining 0x00, /* minor version */ 0x03, 0x02, 0xf7]) {
                ]) {
                    *payload = None
                } else {
                    *payload = Some(DeviceError::WrongDeviceId { id: msg.to_vec() })
                }
            },
        )?;
        self.midi_connection
            .send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])?;

        if let Some(err) = sysex_replies.close_wait(500) {
            return Err(Box::new(err));
        }
        self.sysex_counter += 1;
        Ok(())
    }
}

impl Device for MicroBruteDevice {
    fn query(&mut self, params: &[String]) -> Result<Vec<(String, String)>> {
        let sysex_replies = devices::sysex_query_init(
            &self.port_name,
            || vec![],
            |_ts, msg, payload| {
                let pcode = msg[8];
                let vcode = msg[9];
                if let Some(param) = sysex_reply_code(pcode) {
                    if let Some(value_str) = bound_str(bounds(&param), vcode) {
                        payload.push((param.as_ref().to_string(), value_str));
                    }
                }
            },
        )?;
        for param in params {
            let p = MicrobruteParameter::from_str(param)?;
            self.midi_connection
                .send(&sysex_query_msg(self.sysex_counter, sysex_query(p)))?;
            self.sysex_counter += 1;
        }
        Ok(sysex_replies.close_wait(500))
    }

    fn update(&mut self, param_id: &str, value_id: &str) -> Result<()> {
        let param = MicrobruteParameter::from_str(param_id)?;
        let v = bound_code(bounds(&param), value_id)
            .ok_or(DeviceError::ValueOutOfBound { value_name: value_id.to_string() })?;
        self.midi_connection
            .send(&sysex_update_msg(self.sysex_counter, sysex_update(param), v))?;
        self.sysex_counter += 1;
        Ok(())
    }
}

/// The sysex parameter code without a following value.
/// Used when querying device for current setting.
/// The code is equal to the param's associated update_code + 1
fn sysex_query(param: MicrobruteParameter) -> u8 {
    match param {
        KeyNotePriority => 0x0c,
        KeyVelocityResponse => 0x12,
        MidiRecvChan => 0x06,
        MidiSendChan => 0x08,
        LfoKeyRetrig => 0x10,
        EnvLegatoMode => 0x0e,
        BendRange => 0x2d,
        Gate => 0x37,
        Sync => 0x3d,
        SeqPlay => 0x2f,
        SeqKeyRetrig => 0x35,
        SeqNextSeq => 0x33,
        SeqStepOn => 0x2b,
        SeqStep => 0x39,
    }
}

fn sysex_update(param: MicrobruteParameter) -> u8 {
    match param {
        KeyNotePriority => 0x0b,
        KeyVelocityResponse => 0x11,
        MidiSendChan => 0x07,
        MidiRecvChan => 0x05,
        LfoKeyRetrig => 0x0f,
        EnvLegatoMode => 0x0d,
        BendRange => 0x2c,
        Gate => 0x36,
        Sync => 0x3c,
        SeqPlay => 0x2e,
        SeqKeyRetrig => 0x34,
        SeqNextSeq => 0x32,
        SeqStepOn => 0x2a,
        SeqStep => 0x38,
    }
}

fn sysex_reply_code(code: u8) -> Option<MicrobruteParameter> {
    match code {
        0x0b => Some(KeyNotePriority),
        0x11 => Some(KeyVelocityResponse),
        0x07 => Some(MidiSendChan),
        0x05 => Some(MidiRecvChan),
        0x0f => Some(LfoKeyRetrig),
        0x0d => Some(EnvLegatoMode),
        0x2c => Some(BendRange),
        0x36 => Some(Gate),
        0x3c => Some(Sync),
        0x2e => Some(SeqPlay),
        0x34 => Some(SeqKeyRetrig),
        0x32 => Some(SeqNextSeq),
        0x2a => Some(SeqStepOn),
        0x38 => Some(SeqStep),
        _ => None,
    }
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
