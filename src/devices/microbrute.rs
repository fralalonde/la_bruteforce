use crate::devices::Bounds::*;
use crate::devices::{MidiValue, Bounds, Device, Parameter, Descriptor};
use crate::midi::{CLIENT_NAME};
use midi::Result;
use midir::{MidiOutput, MidiOutputConnection};
use strum::IntoEnumIterator;
use crate::midi::{self, MidiPort};
use crate::devices::DeviceError;
use std::str::FromStr;
use self::MicrobruteParameter::*;

//            usb_vendor_id: 0x1c75,
//            usb_product_id: 0x0206,
//            sysex_tx_id: 0x06,

//const MICROBRUTE_SYSEX_REQUEST: u8 = 0x06;
//const MICROBRUTE_SYSEX_REPLY: u8 = 0x05;

#[derive(Debug, EnumString, IntoStaticStr, EnumIter, AsRefStr)]
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
pub struct MicroBruteDescriptor {
}

impl Descriptor for MicroBruteDescriptor {

    fn parameters(&self) -> Vec<Parameter> {
        MicrobruteParameter::iter().map(|p| p.into()).collect()
    }

    fn bounds(&self, param: &str) -> Result<Bounds> {
        bounds(param)
    }

    fn ports(&self) -> Vec<MidiPort> {
        let midi_client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
        midi::output_ports(&midi_client).into_iter()
            .filter_map(|port| if port.name.starts_with("MicroBrute") {Some(port)} else {None} )
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

fn bounds(param: &str) -> Result<Bounds> {
    let p = MicrobruteParameter::from_str(param)?;
    Ok(match p {
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
    })
}

pub struct MicroBruteDevice {
    midi_connection: MidiOutputConnection,
    port_name: String,
    sysex_counter: usize,
}

impl MicroBruteDevice {

    // TODO return device version / id string
    fn identify(&mut self) -> Result<()> {
        let sysex_replies = midi::sysex_query_init(&self.port_name)?;
        self.midi_connection.send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])?;
        let id = sysex_replies.close_wait(500).get(0).cloned().ok_or(DeviceError::NoReply)?;

        if !id.as_slice().starts_with(&[0xf0, 0x7e, 0x01, /* arturia1 */ 0x06, /* arturia2 */ 0x02, 0x00, 0x20, 0x6b, 0x04, 0x00, 0x02, 0x01, /* major version */ 0x01, 0x00]) {
            // remaining 0x00, /* minor version */ 0x03, 0x02, 0xf7]) {
             Err(Box::new(DeviceError::WrongId {id: id.to_vec()}))
        } else {
            self.sysex_counter += 1;
            Ok(())
        }
    }
}

impl Device for MicroBruteDevice {
    fn query(&mut self, params: &[String]) -> Result<Vec<(Parameter, MidiValue)>> {
        let sysex_replies = midi::sysex_query_init(&self.port_name)?;
        for param in params {
            let p = MicrobruteParameter::from_str(param)?;
            self.midi_connection.send(&sysex_query_msg(self.sysex_counter, sysex_request(p)))?;
            self.sysex_counter += 1;
        }
        Ok(sysex_replies.close_wait(500).iter()
            .map(|msg| {
                let z = sysex_reply(msg[8]);
                let p = match z {
                    Some(p) => p.into(),
                    None => "Unknown param"
                };
                (p, msg[9])
            })
            .collect()
        )
    }

    fn update(&mut self, param: &str, value: &str) -> Result<()> {
        let p = MicrobruteParameter::from_str(param)?;
        let v = match bounds(param)? {
            Bounds::Discrete(values) => values.iter()
                .find(|d| d.1.eq(value))
                .expect("FUCK RUST ERRORS").0,
            Bounds::Range(offset, (_lo, _hi)) => u8::from_str(value).expect("FUCK RUST ERRORS") - offset,
        };
        self.midi_connection
            .send(&sysex_update_msg(self.sysex_counter, sysex_request(p), v))?;
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


fn sysex_request(param: MicrobruteParameter) -> u8 {
    match param {
        KeyNotePriority => 0x0c,
        KeyVelocityResponse => 0x12,
        MidiRecvChan => 0x06,
        MidiSendChan => 0x08,
        LfoKeyRetrig => 0x10,
        EnvLegatoMode => 0x0e,
        BendRange => 0x2b,
        Gate => 0x37,
        Sync => 0x3d,
        SeqPlay => 0x2f,
        SeqKeyRetrig => 0x35,
        SeqNextSeq => 0x33,
        SeqStep => 0x39,
        SeqStepOn => 0x2d,
    }
}

fn sysex_reply(code: u8) -> Option<MicrobruteParameter> {
    match code {
        0x0b => Some(KeyNotePriority),
        0x11 => Some(KeyVelocityResponse),
        0x05 => Some(MidiRecvChan),
        0x07 => Some(MidiSendChan),
        0x0e => Some(LfoKeyRetrig),
        0x0d => Some(EnvLegatoMode),
        0x2c => Some(BendRange),
        0x36 => Some(Gate),
        0x3c => Some(Sync),
        0x2e => Some(SeqPlay),
        0x34 => Some(SeqKeyRetrig),
        0x32 => Some(SeqNextSeq),
        0x38 => Some(SeqStep),
        0x2a => Some(SeqStepOn),
        _ => None
    }
}

//    if is_device_sysex(message, MICROBRUTE_SYSEX_REPLY) {
//        let param_id = message[8];
//        let value = message[9] as MidiValue;
//        received_values.insert(param_id, value);
//    }


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


