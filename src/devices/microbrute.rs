use crate::devices::Bounds::*;
use crate::devices::CLIENT_NAME;
use crate::devices::{self, MidiPort};
use crate::devices::{sysex, DeviceError, MidiNote, ARTURIA, IDENTITY_REPLY};
use crate::devices::{Bounds, Descriptor, Device};
use crate::schema;

use devices::Result;
use hex;
use midir::{MidiOutput, MidiOutputConnection};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::IntoEnumIterator;
use linked_hash_map::LinkedHashMap;
use regex::Regex;

// usb_vendor_id: 0x1c75,
// usb_product_id: 0x0206,

static MICROBRUTE: &[u8] = &[0x00, 0x20, 0x6b, 0x05];

const REST_NOTE: u8 = 0x7f;

use crate::schema::Parameter;

struct MicrobruteGlobals {
    parameter: Parameter,
    index: Option<usize>,
}

impl MicrobruteGlobals {
    fn sysex_query_code(&self) -> [u8; 2] {
        let z = self.sysex_data_code();
        [z[0], z[1] + 1]
    }

    /// low index is always 1
    fn max_index(&self) -> Option<usize> {
        self.parameter.range.map(|i| i.hi)
    }

}

pub struct MicroBruteDevice {
    schema: schema::Device,
    midi_connection: MidiOutputConnection,
    port_name: String,
    msg_id: usize,
}

impl MicroBruteDevice {z
    /// from CLI
    fn parse(&self, param_str: &str) -> Result<Parameter> {
        let re = Regex::new(r"(?P<name>.+)(:?/(?P<seq>\d+))(?::(?P<mode>.+))")?;
        if let Some(cap) = re.captures(param_str) {
            let pname = cap.name("name")?;
            Parameter {
                self.schema.parameters.get()?;
            }
        }

        let mut parts = s.split("/");
        if let Some(name) = parts.next() {
            if let Some(idx_or_mode) = parts.next() {
                let mut idx_mode = idx_or_mode.split(":");
                if let Some(mode) = parts.next() {
                    // idx starts from 1, internally starts from 0
                    let idx = u8::from_str(idx)? - 1;
                    match name {
                        "Seq" => Ok(Seq(idx)),
                        _ => Err(Box::new(DeviceError::UnknownParameter {
                            param_name: s.to_owned(),
                        })),
                    }
                }
            } else {
                Ok(MicrobruteGlobals::from_str(s)?)
            }
        } else {
            return Err(Box::new(DeviceError::EmptyParameter));
        }
    }

    fn bounds(param: MicrobruteGlobals) -> Bounds {
    }

    fn bound_reqs(bounds: MicrobruteGlobals) -> (usize, usize) {
    }

    // TODO return device version / id string

}

impl Device for MicroBruteDevice {
    fn query(&mut self, params: &[String]) -> Result<LinkedHashMap<String, Vec<String>>> {
        let sysex_replies = devices::sysex_query_init(&self.port_name, MICROBRUTE, decode)?;
        for param_str in params {
            let param = MicrobruteGlobals::parse(param_str)?;
            let query_code = &param.sysex_query_code();
            match param.index() {
                Some(idx) => {
                    //0x01 MSGID(u8) 0x03,0x3b(SEQ) SEQ_IDX(u8 0 - 7) 0x00 SEQ_OFFSET(u8) SEQ_LEN(0x20)
                    self.midi_connection.send(&sysex(
                        MICROBRUTE,
                        &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x00, 0x20]],
                    ))?;
                    self.msg_id += 1;
                    self.midi_connection.send(&sysex(
                        MICROBRUTE,
                        &[&[0x01, self.msg_id as u8], query_code, &[idx, 0x20, 0x20]],
                    ))?;
                    self.msg_id += 1;
                }
                None => {
                    self.midi_connection.send(&sysex(
                        MICROBRUTE,
                        &[&[0x01, self.msg_id as u8], query_code],
                    ))?;
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
                // 0x01 MSGID(u8) SEQ(0x23, 0x3a) SEQ_ID(u8) SEQ_OFFSET(u8) SEQ_LEN(u8, max 0x20) SEQ_NOTES([u8; 32] 0 padded, start@ C0=0x30, C#0 0x31... rest=0x7f)
                let mut seqlen = bcodes.len() as u8;
                for _padding in 0..(64 - bcodes.len()) {
                    bcodes.push(0x00)
                }
                static BLOCK_SIZE: u8 = 0x20;
                for block in 0..1 {
                    let offset: usize = BLOCK_SIZE as usize * block;
                    self.midi_connection.send(&sysex(
                        MICROBRUTE,
                        &[
                            &[0x01, self.msg_id as u8],
                            &param.sysex_data_code(),
                            &[
                                seq_idx,
                                offset as u8,
                                if seqlen > BLOCK_SIZE {
                                    BLOCK_SIZE
                                } else {
                                    seqlen
                                },
                            ],
                            &bcodes[offset..offset + BLOCK_SIZE as usize],
                        ],
                    ))?;
                    if seqlen > BLOCK_SIZE {
                        seqlen -= BLOCK_SIZE;
                    }
                    self.msg_id += 1;
                }
            }
            _ => {
                self.midi_connection.send(&sysex(
                    MICROBRUTE,
                    &[
                        &[0x01, self.msg_id as u8],
                        &param.sysex_data_code(),
                        &[*bcodes.get(0).ok_or(DeviceError::MissingValue {
                            param_name: param_str.to_string(),
                        })?],
                    ],
                ))?;
                self.msg_id += 1;
            }
        }
        Ok(())
    }
}

fn decode(msg: &[u8], result_map: &mut LinkedHashMap<String, Vec<String>>) {
    if let Some(param) = into_param(msg) {
        match param {
            Seq(_idx) => {
                let notes = result_map.entry(param.to_string()).or_insert(vec![]);
                for nval in &msg[7..] {
                    if *nval == 0 {
                        break;
                    }
                    if *nval == REST_NOTE {
                        notes.push("_".to_string());
                    } else if *nval < 24 {
                        notes.push(format!("?{}", *nval));
                    } else {
                        notes.push(MidiNote { note: *nval - 24 }.to_string());
                    }
                }
            }
            param => {
                if let Some(bound) = devices::bound_str(bounds(param), &[msg[4]]) {
                    let _ = result_map.insert(param.to_string(), vec![bound]);
                } else {
                    eprintln!(
                        "param {} unbound value code '{}'",
                        param.to_string(),
                        msg[4]
                    );
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
                _ => return Some(p),
            }
        }
    }
    None
}

#[cfg(test)]
mod test {
    use regex::Regex;

    #[test]
    fn beurk() {
        let re = Regex::new(r"(?P<name>.+)(:?/(?P<seq>\d+))(?::(?P<mode>.+))")?;

        let mut text = "Param/4:Mode";
        let matches =re.captures(text).unwrap();
        assert!{matches.name("name").eq(&Some("Param"))}
        assert!{matches.name("seq").eq(&Some("4"))}
        assert!{matches.name("mode").eq(&Some("Mode"))}

        text = "Param/4";
        let matches =re.captures(text).unwrap();
        assert!{matches.name("name").eq(&Some("Param"))}
        assert!{matches.name("seq").eq(&Some("4"))}
        assert!{matches.name("mode").eq(&None)}

        let text = "Param:Mode";
        let matches =re.captures(text).unwrap();
        assert!{matches.name("name").eq(&Some("Param"))}
        assert!{matches.name("seq").eq(&None)}
        assert!{matches.name("mode").eq(&Some("Mode"))}

        let mut text = "Param";
        let matches =re.captures(text).unwrap();
        assert!{matches.name("name").eq(&Some("Param"))}
        assert!{matches.name("seq").eq(&None)}
        assert!{matches.name("mode").eq(&None)}

    }
}