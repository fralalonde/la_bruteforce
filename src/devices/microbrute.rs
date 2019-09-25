use self::MicrobruteGlobals::*;
use crate::devices::Bounds::*;
use crate::devices::DeviceError;
use crate::devices::{Bounds, Descriptor, Device};
use crate::devices::CLIENT_NAME;
use crate::devices::{self, MidiPort};

use devices::Result;
use midir::{MidiOutput, MidiOutputConnection};
use std::str::FromStr;
use strum::{IntoEnumIterator};

//            usb_vendor_id: 0x1c75,
//            usb_product_id: 0x0206,

//const MICROBRUTE_SYSEX_REQUEST: u8 = 0x06;

//static ARTURIA: &[u8] = &[0x00, 0x20, 0x6b];

// UPDATE SEQ
//0x01 MSGID(u8) SEQ(0x23, 0x3a) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)

//01 37 23 3a 00 00 01 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//01 38 23 3a 01 00 02 30 31 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//01 39 23 3a 02 00 04 30 31 7f 32 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//01 3a 23 3a 03 00 20 3c 30 7f 48 3c 7f 48 7f 3c 30 7f 48 3c 7f 48 7f 3c 30 7f 48 3c 7f 48 7f 3f 33 7f 3f 33 7f 41 7f
//01 3b 23 3a 04 00 20 30 3c 48 46 30 43 3c 37 30 7f 41 48 30 3e 48 3e 33 3a 3f 48 33 46 3f 3a 33 7f 3e 48 33 48 3e 40
//01 3c 23 3a 05 00 20 30 7f 7f 3c 30 7f 3c 7f 3c 7f 7f 48 3c 7f 3c 7f 30 7f 7f 3c 30 7f 3c 7f 41 7f 7f 41 7f 7f 44 7f

//01 seq 23 3a 05 20 20 30 7f 7f 3c 30 7f 3c 7f 3c 7f 7f 48 3c 7f 3c 7f 30 7f 7f 3c 30 7f 3c 7f 35 41 34 40 33 3f 32 31
//01 seq 23 3a 06 00 20 3c 48 30 3c 3c 7f 7f 30 3c 48 7f 3c 3c 7f 7f 49 30 48 3c 3d 3c 7f 48 54 48 30 4f 52 30 4d 51 46
//01 seq 23 3a 07 00 20 30 30 7f 7f 48 3c 30 30 7f 7f 30 7f 48 7f 52 7f 30 30 7f 7f 48 3c 30 30 7f 7f 30 7f 48 7f 4b 7f
//01 seq 23 3a 07 20 20 30 30 7f 7f 48 31 30 30 7f 7f 3c 7f 48 7f 52 7f 30 30 7f 7f 7f 7f 30 30 7f 30 30 7f 7f 30 7f 7f

// QUERY
//inquiry1  01 59 00 37
//reply     01 59 01 36 02 01 00 00 00 00 00 00 00  // 01 = major version?
//inquiry2  01 5a 00 39
//reply     01 5a 01 38 08 04 00 00 00 00 00 00 00  // 04 = minor version?

// SEQ 1
//0x01 MSGID(u8) 0x03,0x3b(SEQ) SEQ_IDX(u8 0 - 7) 0x00 SEQ_OFFSET(u8) SEQ_LEN(0x20)
//getseq1   01 5b 03 3b 00 00 20
// repeat with offset 0x20 for notes 33-64 (maybe)
//getseq2   01 5c 03 3b 00 20 20

//0x01 MSGID(u8) SEQ(0x23) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)
//repseq1a   01 5b 23 3a 00 00 20 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 3c 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//repseq1b   01 5c 23 3a 00 20 20 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//
//repseq2a   01 32 23 3a 01 00 20 3c 3c 3c 30 3c 3c 3c 48 3c 3c 3c 30 3c 3c 48 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
//repseq2a   01 33 23 3a 01 20 20 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00


//0x01 MSGID(u8) SEQ(0x23, 0x3a) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start C0 0x30, C#0 0x31)

static MICROBRUTE: &[u8] = &[0x00, 0x20, 0x6b, 0x05];

static REALTIME: u8 = 0x7e;

static IDENTITY_REPLY: &[u8] = &[REALTIME, 0x01, 0x06, 0x02];

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

impl From<MicrobruteGlobals> for u8 {
    fn from(g: MicrobruteGlobals) -> Self {
        match g {
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
            Seq(_) => 0x3a,
        }
    }
}

impl MicrobruteGlobals {
    /// low index is always 1
    fn max_index(&self) -> Option<usize> {
        match self {
            Seq(_) => Some(8),
            _ => None,
        }
    }

    fn parse(s: &str) -> Result<Self> {
        Ok(MicrobruteGlobals::from_str(s).unwrap_or_else(|_err|  {
            let parts: Vec<&str> = s.split("/").collect();
            match &parts[..] {
                [param, idx] => Seq(1/*u8::from_str(idx)?*/),
                _ => Seq(2)
            }
        }))
    }
}

#[derive(Debug)]
pub struct MicroBruteDescriptor {}

impl Descriptor for MicroBruteDescriptor {
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
        Seq(_) => NoteSeq
    }
}

pub struct MicroBruteDevice {
    midi_connection: MidiOutputConnection,
    port_name: String,
    msg_id: usize,
}

impl MicroBruteDevice {
    // TODO return device version / id string
    fn identify(&mut self) -> Result<()> {
        let sysex_replies = devices::sysex_query_init(&self.port_name, IDENTITY_REPLY)?;
        self.midi_connection
            .send(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])?;
        let id = sysex_replies.close_wait(500)
            .get(0)
            .cloned()
            .ok_or(DeviceError::NoReply)?;

        if !id.as_slice().starts_with(&[
            0xf0, 0x7e, 0x01, 0x06, 0x02, 0x00, /* arturia */ 0x20, 0x6b,
            0x04, 0x00, 0x02, 0x01, /* major version */ 0x01, 0x00,
            // remaining 0x00, /* minor version */ 0x03, 0x02, 0xf7]) {
        ]) {
            Err(Box::new(DeviceError::WrongId { id: id.to_vec() }))
        } else {
            self.msg_id += 1;
            Ok(())
        }
    }
}

impl Device for MicroBruteDevice {
    fn query(&mut self, params: &[String]) -> Result<Vec<(String, String)>> {
        let sysex_replies = devices::sysex_query_init(&self.port_name, MICROBRUTE)?;
        for param_str in params {
            let param = MicrobruteGlobals::parse(param_str)?;
            let p: u8 = param.into();
            self.midi_connection
                .send(&sysex(MICROBRUTE, &[ 0x01, self.msg_id as u8, 0x00, p + 1]))?;
            self.msg_id += 1;
        }
        Ok(sysex_replies.close_wait(500).iter()
            .filter_map(|msg| decode(msg[8], msg[9]))
            .collect())
    }

    fn update(&mut self, param_str: &str, value_ids: &[String]) -> Result<()> {
        let param = MicrobruteGlobals::parse(param_str)?;
        let bounds = bounds(param);
        let mut body = vec![ 0x01, self.msg_id as u8, 0x01, param.into() ];
        body.append(&mut devices::bound_codes(bounds, value_ids)?);
        self.midi_connection.send(&sysex(MICROBRUTE, &body))?;
        self.msg_id += 1;
        Ok(())
    }
}

fn decode(pcode: u8, vcode: u8) -> Option<(String, String)> {
    into_param(pcode)
        .and_then(|param| devices::bound_str(bounds(param), &[vcode])
            .map(|bound| (param.as_ref().to_string(), bound)))
}

fn into_param(code: u8) -> Option<MicrobruteGlobals> {
    for p in MicrobruteGlobals::iter() {
        let pcode: u8 = p.into();
        if pcode == code {
            return Some(p)
        }
    }
    None
}

fn sysex(vendor: &[u8],  payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(2 + vendor.len() + payload.len());
    msg.push(0xf0);
    msg.extend_from_slice(vendor);
    msg.extend_from_slice(payload);
    msg.push(0xf7);
    msg
}
