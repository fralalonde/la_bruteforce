use crate::devices::{DeviceError, MidiNote};
use crate::parse::Token::{Value, Control, Vendor};
use crate::schema;
use std::collections::VecDeque;
use std::mem::take;
use snafu::*;

#[derive(Debug, Snafu)]
pub enum ParseError {
    Unexpected {
//        device_name: String,
    },
}

pub enum Token {
    Vendor(&'static schema::Vendor),
    Device(&'static schema::Device),
    SysexId(usize),
    Patch(usize),

    Control(&'static schema::Control),
    IndexedControl(&'static schema::Control, usize),

    Mode(&'static schema::Value),
    Field(&'static schema::Field),

    Value(&'static schema::Value),
    InRange(&'static schema::Range, isize),
    MidiNotes(&'static schema::MidiNotes, usize, Vec<u8>),
}

const SYSEX_BEGIN: &[u8] = &[0xf0];
const SYSEX_END: &[u8] = &[0xf7];

#[derive(Debug)]
pub struct SysexReply {
    offset: usize,
    tokens: VecDeque<Token>,
    mode: Option<&'static schema::Value>
}

impl SysexReply {
    pub fn new() -> Self {
        SysexReply {
            offset: 0,
            tokens: Vec::with_capacity(6),
            mode: None,
        }
    }

    pub fn parse(&mut self, message: &[u8]) -> Result<Vec<Token>, ParseError> {
        if message.is_empty() {
            return ParseError::EmptyReply;
        }
        self.expect(SYSEX_BEGIN)?;
        self.vendor(message)?;
        self.expect(SYSEX_END)?;
        Ok(self.tokens.drain([..]).collect())
    }

    fn vendor(&mut self, message: &[u8]) -> Result<(), ParseError> {
        for v in VENDORS.iter() {
            if self.accept(v.sysex) {
                self.tokens.push(Token::Vendor(v));
                return self.device(&v.devices, message);
            }
        }
        Err(ParseError::UnknownVendor)
    }

    fn device(&mut self, vendor: &schema::Vendor, message: &[u8]) -> Result<(), ParseError> {
        for d in &vendor.devices {
            if self.accept(d.sysex) {
                self.tokens.push(Token::Device(d));

                self.expect(&[0x01])?; // device sysex id?
                self.take(1)?; // msg id, unused for now
                self.take(1)?; // unknown, ignore (01 for regular param, 23 for sequences)

                return self.control(d, message);
            }
        }
        Err(ParseError::UnknownDevice)
    }

    fn control(&mut self, device: &schema::Device, message: &[u8]) -> Result<(), ParseError> {
        if let Some(controls) = &device.controls {
            for c in controls {
                if self.accept(c.sysex) {
                    self.tokens.push(Token::Control(c));
                    return self.bounds(&c.bounds, message);
                }
            }
        }
        if let Some(controls) = &device.indexed_controls {
            for c in controls {
                if self.accept(c.sysex) {
                    // could decompose into index() if other tokens need it e.g. device
                    let index = self.take(1)?[0] as usize;
                    self.tokens.push(Token::IndexedControl(c, index));
                    return self.bounds(&c.bounds, message);
                }
            }
        }

        // TODO indexed modal controls

        Err(ParseError::UnknownControl)
    }

    fn bounds(&mut self, bounds: &[schema::Bounds], message: &[u8]) -> Result<(), ParseError> {
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => values(values),
                schema::Bounds::Range(range) => in_range(range),
                schema::Bounds::MidiNotes(seq) => note_seq(seq),
            };
            if let Some(token) = check {
                self.tokens.push(token);
                return Ok(())
            }
        }
        Err(ParseError::NoMatchingBounds)
    }

    fn values(&mut self, values: &[schema::Value]) -> Option<Token> {
        let value = self.take(1)?[0];
        for v in &values {
            if v.sysex == value {
                return Some(Token::Value(v));
            }
        }
        None
    }

    fn in_range(&mut self, range: &schema::Range) -> Option<Token> {
        let mut value = self.take(1)?[0] as isize;
        if value >= range.lo && value <= range.hi {
            if let Some(offset) = range.offset {
                value += offset;
            }
            return Some(Token::InRange(range, value))
        }
        None
    }

    fn note_seq(&mut self, range: &schema::MidiNotes) -> Option<Token> {
        let start_offset = self.take(1)?[0] as usize;
        let seq_length = self.take(1)?[0] as usize;
        let notes = self.take(seq_length)?.iter()
            .map(|note|  MidiNote {
                note: *if let Some(offset) = seq.offset {
                    note + offset
                } else {
                    note
                }
            }).collect();
        Some(Token::MidiNotes(seq, start_offset, notes))
    }


    #[inline]
    fn select(&mut self, length: usize) -> Option<&[u8]> {
        let end = self.offset + value.len();
        if end <= self.msg.len() {
            let token = &self.msg[self.offset..end];
            Some(token)
        } else {
            None
        }
    }

    fn accept(&mut self, value: &[u8]) -> bool {
        if let Some(token) = self.take(value.len()) {
            if token.eq(value) {
                self.offset += value.len();
                return true;
            }
        }
        false
    }

    fn take(&mut self, length: usize) -> Result<&[u8], ParseError> {
        if let Some(token) = self.select(length) {
            self.offset += length;
            Ok(token)
        } else {
            Err(ParseError::ShortRead)
        }
    }

    fn expect(&mut self, value: &[u8]) -> Result<(), MessageParseError> {
        Ok(if !self.accept(value) {
            ParseError::Unexpected
        }?)
    }
}

pub fn parse_query(input: &str) -> Result<Vec<Token>, ParseError> {
    if query.is_empty() {
        return ParseError::EmptyQuery;
    }
    let mut query = TextQuery::new();
    query.device(input)?;
    Ok(query.tokens.drain([..]).collect())
}

#[derive(Debug)]
struct TextQuery {
    offset: usize,
    tokens: VecDeque<Token>,
    mode: Option<&'static schema::Value>
}

impl TextQuery {
    pub fn new() -> Self {
        TextQuery {
            offset: 0,
            tokens: Vec::with_capacity(6),
            mode: None,
        }
    }

    fn device(&mut self, input: &str) -> Result<(), ParseError> {
        for d in &DEVICES {
            if self.accept(d.name) {
                self.tokens.push(Token::Device(d));
                self.expect("/")?;
                self.expect(&[0x01])?; // device sysex id?
                self.take(1)?; // msg id, unused for now
                self.take(1)?; // unknown, ignore (01 for regular param, 23 for sequences)

                return self.control(d, message);
            }
        }
        Err(ParseError::UnknownDevice)
    }

    fn control(&mut self, device: &schema::Device, input: &str) -> Result<(), ParseError> {
        if let Some(controls) = &device.controls {
            for c in controls {
                if accept(c.sysex) {
                    self.tokens.push(Token::Control(c));
                    return self.bounds(&c.bounds, message);
                }
            }
        }
        if let Some(controls) = &device.indexed_controls {
            for c in controls {
                if accept(c.sysex) {
                    // could decompose into index() if other tokens need it e.g. device
                    let index = self.take(1)?[0] as usize;
                    self.tokens.push(Token::IndexedControl(c, index));
                    return self.bounds(&c.bounds, message);
                }
            }
        }

        // TODO indexed modal controls

        Err(ParseError::UnknownControl)
    }

    fn bounds(&mut self, bounds: &[schema::Bounds], input: &str) -> Result<(), ParseError> {
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => values(values),
                schema::Bounds::Range(range) => in_range(range),
                schema::Bounds::MidiNotes(seq) => note_seq(seq),
            };
            if let Some(token) = check {
                self.tokens.push(token);
                return Ok(())
            }
        }
        Err(ParseError::NoMatchingBounds)
    }

    fn values(&mut self, values: &[schema::Value]) -> Option<Token> {
        let value = self.take(1)?[0];
        for v in &values {
            if v.sysex == value {
                return Some(Token::Value(v));
            }
        }
        None
    }

    fn in_range(&mut self, range: &schema::Range) -> Option<Token> {
        let mut value = self.take(1)?[0] as isize;
        if value >= range.lo && value <= range.hi {
            if let Some(offset) = range.offset {
                value += offset;
            }
            return Some(Token::InRange(range, value))
        }
        None
    }

    fn note_seq(&mut self, range: &schema::MidiNotes) -> Option<Token> {
        let start_offset = self.take(1)?[0] as usize;
        let seq_length = self.take(1)?[0] as usize;
        let notes = self.take(seq_length)?.iter()
            .map(|note|  MidiNote {
                note: *if let Some(offset) = seq.offset {
                    note + offset
                } else {
                    note
                }
            }).collect();
        Some(Token::MidiNotes(seq, start_offset, notes))
    }


    #[inline]
    fn select(&mut self, length: usize) -> Option<&str> {
        let end = self.offset + value.len();
        if end <= self.msg.len() {
            let token = &self.msg[self.offset..end];
            Some(token)
        } else {
            None
        }
    }

    fn accept(&mut self, ident: &str) -> bool {
        if let Some(token) = self.take(value.len()) {
            if token.eq(ident) {
                self.offset += value.len();
                return true;
            }
        }
        false
    }

    fn take(&mut self, length: usize) -> Result<&[u8], ParseError> {
        if let Some(token) = self.select(length) {
            self.offset += length;
            Ok(token)
        } else {
            Err(ParseError::ShortRead)
        }
    }

    fn expect(&mut self, ident: &str) -> Result<(), MessageParseError> {
        Ok(if !self.accept(ident) {
            ParseError::Unexpected
        }?)
    }
}
