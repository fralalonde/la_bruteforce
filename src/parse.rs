use crate::devices::{DeviceError, MidiNote};
use crate::parse::Token::{Value, Control, Vendor};
use crate::schema;
use std::collections::VecDeque;
use snafu::*;
use std::str::FromStr;
use std::num::ParseIntError;

#[derive(Debug, Snafu)]
pub enum ParseError {
    Unexpected,
    UnknownVendor,
    UnknownDevice,
    UnknownControl,
    NoMatchingBounds,
    ShortRead,
    EmptyMessage,
    EmptyQuery,
    MissingControl,
    MissingControlName,
    BadControlSyntax,
    BadControlIndex,
    MissingValue,
}

// TODO SNAFUize this
impl From<std::num::ParseIntError> for ParseError {
    fn from(_: ParseIntError) -> Self {
        unimplemented!()
    }
}

#[derive(Debug)]
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
    MidiNotes(&'static schema::MidiNotes, usize, Vec<MidiNote>),
}

const SYSEX_BEGIN: &[u8] = &[0xf0];
const SYSEX_END: &[u8] = &[0xf7];

#[derive(Debug)]
pub struct SysexReply {
//    offset: usize,
    tokens: Vec<Token>,
    mode: Option<&'static schema::Value>
}

impl SysexReply {
    pub fn new() -> Self {
        SysexReply {
//            offset: 0,
            tokens: Vec::with_capacity(6),
            mode: None,
        }
    }

    pub fn parse(&mut self, message: &mut [u8]) -> Result<Vec<Token>, ParseError> {
//        self.offset = 0;
        if message.is_empty() {
            return Err(ParseError::EmptyMessage);
        }
        self.expect(SYSEX_BEGIN, message)?;
        self.vendor(message)?;
        self.expect(SYSEX_END, message)?;
        Ok(self.tokens.drain(..).collect())
    }

    fn vendor(&mut self, message: &mut [u8]) -> Result<(), ParseError> {
        for v in schema::VENDORS.values() {
            if self.accept(&v.sysex, message) {
                self.tokens.push(Token::Vendor(v));
                return self.device(v, message);
            }
        }
        Err(ParseError::UnknownVendor)
    }

    fn device(&mut self, vendor: &'static schema::Vendor, message: &mut [u8]) -> Result<(), ParseError> {
        for d in &vendor.devices {
            if self.accept(&d.sysex, message) {
                self.tokens.push(Token::Device(d));

                self.expect(&[0x01], message)?; // device sysex id?
                self.next_byte(message)?; // msg id, unused for now
                self.next_byte(message)?; // unknown, ignore (01 for regular param, 23 for sequences)

                return self.control(d, message);
            }
        }
        Err(ParseError::UnknownDevice)
    }

    fn control(&mut self, device: &'static schema::Device, message: &mut [u8]) -> Result<(), ParseError> {
        if let Some(controls) = &device.controls {
            for c in controls {
                if self.accept(&c.sysex, message) {
                    self.tokens.push(Token::Control(c));
                    return self.bounds(&c.bounds, message);
                }
            }
        }
        if let Some(controls) = &device.indexed_controls {
            for c in controls {
                if self.accept(&c.sysex, message) {
                    // could decompose into index() if other tokens need it e.g. device
                    let index = self.next_byte(message)? as usize;
                    self.tokens.push(Token::IndexedControl(c, index));
                    return self.bounds(&c.bounds, message);
                }
            }
        }

        // TODO indexed modal controls

        Err(ParseError::UnknownControl)
    }

    fn bounds(&mut self, bounds: &'static [schema::Bounds], message: &mut [u8]) -> Result<(), ParseError> {
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => self.values(values, message),
                schema::Bounds::Range(range) => self.in_range(range, message),
                schema::Bounds::MidiNotes(seq) => {
                    let start_offset = self.next_byte(message)? as usize;
                    let seq_length = self.next_byte(message)? as usize;
                    self.note_seq(start_offset, seq_length, seq, message)
                },
            };
            if let Some(token) = check {
                self.tokens.push(token);
                return Ok(())
            }
        }
        Err(ParseError::NoMatchingBounds)
    }

    fn values(&mut self, values: &'static [schema::Value], message: &mut [u8]) -> Option<Token> {
        self.next_byte(message).
            ok()
            .and_then(|value| {
                for v in values {
                    if v.sysex.eq(&value) {
                        return Some(Token::Value(v));
                    }
                }
                None
            })
    }

    fn in_range(&mut self, range: &'static schema::Range, message: &mut [u8]) -> Option<Token> {
        self.next_byte(message).ok()
//            .and_then(|x| x.get(0))
//            .map(|x| *x as isize)
            .and_then(|value| {
                let mut value = value as isize;
                if value >= range.lo && value <= range.hi {
                    if let Some(offset) = range.offset {
                        value += offset;
                    }
                    return Some(Token::InRange(range, value))
                }
                None
            }
        )
    }

    fn note_seq(&mut self, start_offset: usize, seq_length: usize, range: &'static schema::MidiNotes, message: &mut [u8]) -> Option<Token> {
        let pitch_offset = range.offset.unwrap_or(0);
        if let Ok(deez_notez) = self.take(seq_length, message) {
            let mut notes = vec![];
            for z in deez_notez {
                notes.push(MidiNote{note: (z as i16 + pitch_offset) as u8})
            }
            return Some(Token::MidiNotes(range, start_offset, notes))
        }
        None
    }

    fn accept(&mut self, value: &[u8], mut message: &mut [u8]) -> bool {
        if let Ok(token) = self.take(value.len(), message) {
            if token.eq(&value) {
                message = &mut message[value.len()..];
                return true;
            }
        }
        false
    }

    fn take(&mut self, length: usize, message: &mut [u8]) -> Result<Vec<u8>, ParseError> {
        if message.is_empty() {
            return Err(ParseError::ShortRead)
        };
        let (a, message) = message.split_at_mut(length);
        Ok(a.to_vec())
    }

    fn next_byte(&mut self, message: &mut [u8]) -> Result<u8, ParseError> {
        let (z, message) = message.split_first_mut().ok_or(ParseError::ShortRead)?;
        Ok(*z)
    }


    fn expect(&mut self, value: &[u8], message: &mut [u8]) -> Result<(), ParseError> {
        if self.accept(value, message) {
            Ok(())
        } else {
            Err(ParseError::Unexpected)
        }
    }
}

pub fn parse_query(device: &str, items: &mut [String]) -> Result<Vec<Token>, ParseError> {
    if items.is_empty() {
        return Err(ParseError::EmptyQuery)
    }
    let mut query = TextQuery::new();
    query.device(device, items)?;
    Ok(query.tokens.drain(..).collect())
}

#[derive(Debug)]
struct TextQuery {
    tokens: Vec<Token>,
    mode: Option<&'static schema::Value>
}

const DIGITS: &str = "0123456789";
const WHITESPACE: &str = " \t";

impl TextQuery {
    pub fn new() -> Self {
        TextQuery {
            tokens: Vec::with_capacity(6),
            mode: None,
        }
    }

    fn device(&mut self, device: &str, items: &mut [String]) -> Result<(), ParseError> {
        if let Some((vendor, dev)) = schema::DEVICES.get(device) {
            self.tokens.push(Token::Vendor(vendor));
            self.tokens.push(Token::Device(dev));
            self.control(dev, items)
        } else {
            ParseError::UnknownDevice
        }
    }

    fn control(&mut self, device: &'static schema::Device, items: &mut [String]) -> Result<(), ParseError> {
        let (citem, mut items) = items.split_first_mut().ok_or(Err(ParseError::MissingControl))?;
        let seq_parts: Vec<&str> = citem.split("/").collect();
        let cname = seq_parts.get(0).ok_or(Err(ParseError::MissingControlName))?;
        let mut mode_parts: Vec<&str> = citem.split(":").collect();
        match (seq_parts.len(), mode_parts.len()) {
            (1, 1) => {
                let control = device.controls.find(|c| c.name.eq(cname)).ok_or(ParseError::UnknownControl)?;
                self.tokens.push(Token::Control(control));
                self.bounds(&control.bounds, items)
            },
            (2, 1) => {
                let controls = device.indexed_controls.ok_or(ParseError::UnknownControl)?;
                let control = controls.find(|c| c.name.eq(cname)).ok_or(ParseError::UnknownControl)?;
                let idx = usize::from_str(seq_parts.get(1).unwrap()).map_err(|err| Err(ParseError::BadControlIndex))?;
                self.tokens.push(Token::IndexedControl(control, idx));
                self.bounds(&control.bounds, items)
            },
            // TODO
//            (1, 2) => modal control
//            (2, 2) => modal indexed control
            _ => Err(ParseError::BadControlSyntax)
        }
    }

    fn bounds(&mut self, bounds: &'static [schema::Bounds], items: &mut [String]) -> Result<(), ParseError> {
        let (value, mut items) = items.split_first_mut().ok_or(Err(ParseError::MissingValue))?;
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => self.values(values, value),
                schema::Bounds::Range(range) => self.in_range(range, value),
                schema::Bounds::MidiNotes(seq) => self.note_seq(seq, value),
            };
            if let Some(token) = check {
                self.tokens.push(token);
                return Ok(())
            }
        }
        Err(ParseError::NoMatchingBounds)
    }

    fn values(&mut self, values: &'static [schema::Value], input: &str) -> Option<Token> {
        for v in values {
            if v.name.eq(input) {
                return Some(Token::Value(v));
            }
        }
        None
    }

    fn in_range(&mut self, range: &'static schema::Range, input: &str) -> Option<Token> {
        let mut value = isize::from_str(&input).ok()?;
        if value >= range.lo && value <= range.hi {
            value += range.offset.unwrap_or(0);
            return Some(Token::InRange(range, value))
        }
        None
    }

    fn note_seq(&mut self, range: &'static schema::MidiNotes, input: &str) -> Option<Token> {
        let mut nit = input.split(",");
        let mut notes = vec![];
        for n in nit {
            if n.is_empty() {
                continue
            }
            if let Ok(note) = MidiNote::from_str(n) {
                notes.push(note);
            }
        }
        Some(Token::MidiNotes(range, 0, notes))
    }

//    fn accept(&mut self, ident: &str, mut items: &mut [String]) -> bool {
//        if input.starts_with(ident) {
//            input = &mut input[ident.len()..];
//            return true;
//        }
//        false
//    }
//
    fn take(&mut self, matching: &str, input: &str) -> Result<String, ParseError> {
        let mut i = 0;
        let mut vh = input.chars();
        while let Some(c) = vh.next() {
            matching.contains(c);
            i += 1;
        }
        let (z, input) = input.split_at_mut(i);
        Ok(z.to_string())
    }
//
//    fn expect(&mut self, ident: &str, items: &mut [String]) -> Result<(), ParseError> {
//        if self.accept(ident, input) {
//            Ok(())
//        } else {
//            Err(ParseError::Unexpected)
//        }
//    }
}
