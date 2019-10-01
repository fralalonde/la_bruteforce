use self::MicrobruteGlobals::*;
use crate::devices::Bounds::*;
use crate::devices::{DeviceError, MidiNote, sysex, IDENTITY_REPLY, ARTURIA};
use crate::devices::{Bounds, Descriptor, Device};
use crate::devices::CLIENT_NAME;
use crate::devices::{self, MidiPort};

use devices::Result;
use midir::{MidiOutput, MidiOutputConnection};
use std::str::FromStr;
use strum::{IntoEnumIterator};
use hex;
use std::fmt::{Display, Formatter};
use std::fmt;
use std::ops::Deref;

//pub fn beatstep() -> DeviceDescriptor {
//    DeviceDescriptor {
//        name: "BeatStep",
//        port_name_prefix: "Arturia BeatStep",
//        usb_vendor_id: 0x1c75,
//        usb_product_id: 0x0206,
//        sysex_out_id: 0x05,
//        sysex_tx_id: 0x06,
//        params: vec![],
//    }
//}

static BEATSTEP: &[u8] = &[0x00, 0x20, 0x6b, 0x7f, 0x42];

// Knob/1:Off
// Knob/2:Control Option=Relative1 CC=45 Channel=4 Min:0 Max=127
// Knob/2:NRPN Option=Relative1 CC=45 Channel=4 Min:0 Max=127


// Pad/1:Note Option=Gate Channel=Global Note=G#1

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum EncoderMode {
    Off = 0x00,
    CC,
    NRPN,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Encoder {
    mode: EncoderMode,
    fields: EncoderFields
}

impl Deref for Encoder {
    type Target = EncoderMode;

    fn deref(&self) -> &Self::Target {
        &self.mode
    }
}

#[derive(Clone, Copy)]
union EncoderFields {
    off: EncoderOff,
    control: EncoderCc,
    nrpn: EncoderNrpn,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EncoderOff {}

impl From<EncoderOff> for Encoder {
    fn from(off: EncoderOff) -> Self {
        Encoder {
            mode: EncoderMode::Off,
            fields: EncoderFields{off}
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EncoderCc {
    option: EncoderCcOption,
    channel: u8,
    cc: u8,
    min: u8,
    max: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EncoderNrpn {
    option: EncoderCcOption,
    channel: MidiChannel,
    data_entry: DataEntry,
    lsb: u8,
    msb: u8,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum SingleMidiChannelOrAll {
    Ch1 = 0x00,
    // ...
    All = 0x10,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum EncoderCcOption {
    Absolute = 0x00,
    Relative1,
    Relative2,
    Relative3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EncoderRpn {
    option: EncoderRpnOption,
    channel: u8,
    cc: u8,
    min: u8,
    max: u8,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum EncoderRpnOption {
    Rpn,
    Nrpn,
}

//#[repr(u8)]
//#[derive(Clone, Copy)]
//enum PadMode {
//    Off = 0x00,
//    Mmc,
//    Switched,
//    Note = 0x09 ,
//    ProgramChange,
//}
//
//#[repr(C)]
//#[derive(Clone, Copy)]
//struct PadNoteConfig {
//    option: u8,
//    channel: u8,
//    note: u8,
//}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts(
        (p as *const T) as *const u8,
        ::std::mem::size_of::<T>(),
    )
}

fn u8_slice_as_any<T: Sized>(p: &[u8]) -> Result<&T> {
    // at least make sure there are enough bytes in slice
    if ::std::mem::size_of::<T>() > p.len() {
        Err(Box::new(DeviceError::ReadSizeError))
    } else {
        Ok(unsafe { std::ptr::read(p.as_ptr() as *const _) })
    }
}

//
// arturia  beatstep           field   control (0x70..) value
// 00 20 6b 7f       42 02 00  01      70               09

//controller
//fields
//
//f000206b7f420200027041f7


#[derive(Debug, EnumString, IntoStaticStr, EnumIter, AsRefStr, Clone, Copy)]
enum MicrobruteGlobals {
    KeyNotePriority ,
    KeyVelocityResponse ,
    MidiSendChan ,
    MidiRecvChan ,
    LfoKeyRetrig ,
    EnvLegatoMode ,
    BendRange ,
    Gate ,
    Sync ,
    SeqPlay ,
    SeqKeyRetrig ,
    SeqNextSeq ,
    SeqStepOn ,
    SeqStep ,
    Seq(u8) ,
}

impl Display for MicrobruteGlobals {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())?;
        if let Seq(idx) = self {
            f.write_fmt(format_args!("/{}", idx + 1))?;
        }
        Ok(())
    }
}

impl MicrobruteGlobals {
    fn sysex_data_code(&self) -> [u8; 2] {
        match self {
            KeyNotePriority => [0x01, 0x0b],
            KeyVelocityResponse => [0x01, 0x11],
            MidiSendChan => [0x01, 0x07],
            MidiRecvChan => [0x01, 0x05],
            LfoKeyRetrig => [0x01, 0x0f],
            EnvLegatoMode => [0x01, 0x0d],
            BendRange => [0x01, 0x2c],
            Gate => [0x01, 0x36],
            Sync => [0x01, 0x3c],
            SeqPlay => [0x01, 0x2e],
            SeqKeyRetrig => [0x01, 0x34],
            SeqNextSeq => [0x01, 0x32],
            SeqStepOn => [0x01, 0x2a],
            SeqStep => [0x01, 0x38],
            Seq(_) => [0x23, 0x3a],
        }
    }

    fn sysex_query_code(&self) -> [u8; 2] {
        let z = self.sysex_data_code();
        [z[0], z[1] + 1]
    }

    /// low index is always 1
    fn max_index(&self) -> Option<usize> {
        match self {
            Seq(_) => Some(8),
            _ => None,
        }
    }

    fn index(&self) -> Option<u8> {
        match self {
            Seq(idx) => Some(*idx),
            _ => None
        }
    }

    fn parse(s: &str) -> Result<Self> {
        let mut parts = s.split("/");
        if let Some(name) = parts.next() {
            if let Some(idx) = parts.next() {
                // idx starts from 1, internally starts from 0
                let idx = u8::from_str(idx)? - 1;
                match name {
                    "Seq" => Ok(Seq(idx)),
                    _ => Err(Box::new(DeviceError::UnknownParameter {param_name: s.to_owned()}))
                }
            } else {
                Ok(MicrobruteGlobals::from_str(s)?)
            }
        } else {
            return Err(Box::new(DeviceError::EmptyParameter))
        }
    }

}

#[derive(Debug)]
pub struct BeatStepDescriptor {}

impl Descriptor for BeatStepDescriptor {
    fn globals(&self) -> Vec<String> {
        MicrobruteGlobals::iter()
            .flat_map(|p|
                if let Some(max) = p.max_index() {
                    (1..=max).map(|idx| format!("{}/{}", p.as_ref(), idx)).collect()
                } else {
                    vec![p.as_ref().to_string()]
                })
            .collect()
    }

    fn bounds(&self, param: &str) -> Result<Bounds> {
        Ok(bounds(MicrobruteGlobals::parse(param)?))
    }

    fn ports(&self) -> Vec<MidiPort> {
        let midi_client = MidiOutput::new(CLIENT_NAME).expect("MIDI client");
        devices::output_ports(&midi_client)
            .into_iter()
            .filter_map(|port| {
                if port.name.starts_with("BeatStep") {
                    Some(port)
                } else {
                    None
                }
            })
            .collect()
    }

    fn connect(&self, midi_client: MidiOutput, port: &MidiPort) -> Result<Box<dyn Device>> {
        let midi_connection = midi_client.connect(port.number, &port.name)?;
        let mut brute = Box::new(BeatStepDevice {
            midi_connection,
            port_name: port.name.to_owned(),
            msg_id: 0,
        });
        brute.identify()?;
        Ok(brute)
    }
}

fn bounds(param: MicrobruteGlobals) -> Bounds {
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
        Seq(_) => NoteSeq(24)
    }
}

fn bound_reqs(bounds: MicrobruteGlobals) -> (usize, usize) {
    match bounds {
        Seq(_) => (0, 64),
        _ => (1,1)
    }
}

pub struct BeatStepDevice {
    midi_connection: MidiOutputConnection,
    port_name: String,
    msg_id: usize,
}

impl BeatStepDevice {
    // TODO return device version / id string
    fn identify(&mut self) -> Result<()> {
        static ID_KEY: &str = "ID";
        let sysex_replies = devices::sysex_query_init(&self.port_name, IDENTITY_REPLY,
                                                      |msg, result| if msg.starts_with(ARTURIA) {
                                                          // TODO could grab firmware version
                                                          let _ = result.insert(ID_KEY.to_string(), vec![]);
                                                      } else {
                                                          eprintln!("received spurious sysex {}", hex::encode(msg));
                                                      })?;
        self.midi_connection
            .send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])?;
        sysex_replies.close_wait(500)
            .iter().next()
            .ok_or(DeviceError::NoIdentificationReply)?;

        self.msg_id += 1;
        Ok(())
    }
}

impl Device for BeatStepDevice {
    fn query(&mut self, params: &[String]) -> Result<LinkedHashMap<String, Vec<String>>> {
        let sysex_replies = devices::sysex_query_init(&self.port_name, BEATSTEP, decode)?;
        for param_str in params {
            let param = MicrobruteGlobals::parse(param_str)?;
            let query_code  = &param.sysex_query_code();
            match param.index() {
                Some(idx) => {
                    self.midi_connection
                        .send(&sysex(BEATSTEP, &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x00, 0x20]]))?;
                    self.msg_id += 1;
                    self.midi_connection
                        .send(&sysex(BEATSTEP, &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x20, 0x20]]))?;
                    self.msg_id += 1;
                },
                None => {
                    self.midi_connection
                        .send(&sysex(BEATSTEP, &[&[0x01, self.msg_id as u8], query_code]))?;
                    self.msg_id += 1;
                }
            }
        }
        Ok(sysex_replies.close_wait(500))
    }

    fn update(&mut self, param_str: &str, value_ids: &[String]) -> Result<()> {
        let param = MicrobruteGlobals::parse(param_str)?;
        let bounds = bounds(param);
        let reqs = bound_reqs(param);
        let mut bcodes = devices::bound_codes(bounds, value_ids, reqs)?;
        match param {
            Seq(seq_idx) => {
                //0x01 MSGID(u8) SEQ(0x23, 0x3a) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)
                let mut seqlen = bcodes.len() as u8;
                for _padding in 0..(64 - bcodes.len()) {
                    bcodes.push(0x00)
                }
                static BLOCK_SIZE: u8 = 0x20;
                for block in 0..1 {
                    let offset: usize = BLOCK_SIZE as usize * block;
                    self.midi_connection.send(&sysex(BEATSTEP, &[
                        &[ 0x01, self.msg_id as u8 ],
                        &param.sysex_data_code(),
                        &[ seq_idx, offset as u8, if seqlen > BLOCK_SIZE {BLOCK_SIZE} else {seqlen}],
                        &bcodes[offset..offset + BLOCK_SIZE as usize],
                    ]))?;
                    if seqlen > BLOCK_SIZE {
                        seqlen -= BLOCK_SIZE;
                    }
                    self.msg_id += 1;
                }
            }
            _ => {
                self.midi_connection.send(&sysex(BEATSTEP, &[
                    &[ 0x01, self.msg_id as u8 ],
                    &param.sysex_data_code(),
                    &[*bcodes.get(0)
                        .ok_or(DeviceError::MissingValue{param_name: param_str.to_string()})?]
                ]))?;
                self.msg_id += 1;
            }
        }
        Ok(())
    }
}

fn decode(msg: &[u8], result_map: &mut LinkedHashMap<String, Vec<String>>) {
    if let Some(param)  = into_param(msg) {
        match param {
            Seq(_idx) => {
                let notes = result_map.entry(param.to_string()).or_insert(vec![]);
                for nval in &msg[7..] {
                    if *nval == 0 {
                        break;
                    }
                    if *nval < 24 {
                        notes.push(format!("?{}", *nval));
                    }  else {
                        notes.push(MidiNote { note: *nval - 24}.to_string());
                    }
                }
            },
            param => {
                if let Some(bound) = devices::bound_str(bounds(param), &[msg[4]]) {
                    let _ = result_map.insert(param.to_string(), vec![bound]);
                } else {
                    eprintln!("param {} unbound value code '{}'", param.to_string(), msg[4]);
                }
            }
        }
    };
}

fn into_param(msg: &[u8]) -> Option<MicrobruteGlobals> {
    for p in MicrobruteGlobals::iter() {
        if p.sysex_data_code()[1] == msg[3] {
            match p {
                Seq(_) => return Some(Seq(msg[4])),
                _ => return Some(p)
            }
        }
    }
    None
}

