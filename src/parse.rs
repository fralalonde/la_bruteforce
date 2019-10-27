use crate::devices::{DeviceError};
use crate::parse::Token::{Value, Control, Vendor};
use crate::schema;
use std::collections::VecDeque;
use snafu::*;
use std::str::FromStr;
use std::num::ParseIntError;
use crate::schema::MidiNote;

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
    MissingDevice,
    MissingControl,
    MissingControlName,
    BadControlSyntax,
    BadControlIndex,
    MissingValue,
    BadNoteSyntax,
}

// TODO SNAFUize this
impl From<std::num::ParseIntError> for ParseError {
    fn from(_: ParseIntError) -> Self {
        unimplemented!()
    }
}

#[derive(Debug, Snafu)]
pub enum WriteError {

}

#[derive(Debug)]
pub enum Token {
    Vendor(&'static schema::Vendor),
    Device(&'static schema::Device, u8),

//    Patch(usize),

    Control(&'static schema::Control),
    IndexedControl(&'static schema::IndexedControl, u8),

    Mode(&'static schema::Value),
    Field(&'static schema::Field),

    Value(&'static schema::Value),
    InRange(&'static schema::Range, isize),
    MidiNotes(&'static schema::MidiNotes, u8, Vec<MidiNote>),
}

impl Token {
    pub fn to_sysex(&self, buffer: &mut Vec<u8>) {
        match self {
            Token::Vendor(v) => buffer.extend_from_slice(&v.sysex),
            Token::Device(d, idx) => {
                buffer.extend_from_slice(&d.sysex);
                buffer.push(*idx);
            },
            Token::Control(c) => buffer.extend_from_slice(&c.sysex),
            Token::IndexedControl(c, idx) => {
                buffer.extend_from_slice(&c.sysex);
                buffer.push(*idx);
            },

            Token::Mode(m) => buffer.push(m.sysex),
            Token::Field(f) => buffer.extend_from_slice(&f.sysex),

            Token::Value(v) => buffer.extend_from_slice(*v.sysex),
            Token::InRange(r, idx) => buffer.push((*idx) as u8),
            Token::MidiNotes(s, offset, notes) => {
                buffer.push(*offset);
                buffer.push(notes.len() as u8);
                buffer.extend_from_slice(notes.as_ref())
            },
        }
    }
}

pub enum AST {
    Tree(Token, Vec<AST>),
    Chain(Token, Box<AST>),
    Leaf(Token),
}

const SYSEX_BEGIN: &[u8] = &[0xf0];
const SYSEX_END: &[u8] = &[0xf7];


impl AST {
    pub fn to_sysex(&self, msg_id: &mut usize) -> Result<Vec<Vec<u8>>, WriteError> {
        let mut messages: Vec<Vec<u8>> = vec![];
        let mut buffer = SYSEX_BEGIN.to_owned();
        self.to_sysex_inner(&mut buffer, &mut messages);
        Ok(messages)
    }

    fn to_sysex_inner(&self, buffer: &mut Vec<u8>, messages: &mut Vec<Vec<u8>>) {
        match self {
            AST::Tree(token, children) => {
                token.to_sysex(buffer);
                for c in children {
                    let mut buffer = buffer.clone();
                    c.to_sysex_inner(&mut buffer, messages);
                }
            },
            AST::Chain(token, child) => {
                token.to_sysex(buffer);
                child.to_sysex_inner(buffer, messages);
            },
            AST::Leaf(token) => {
                token.to_sysex(buffer);
                buffer.extend_from_slice(SYSEX_END);
                messages.push(buffer.drain(..).collect());
            },
        }
    }
}

#[derive(Debug)]
pub struct SysexReply {
    roots: Vec<AST>,
    mode: Option<&'static schema::Value>
}

impl SysexReply {
    pub fn new() -> Self {
        SysexReply {
            roots: Vec::with_capacity(1),
            mode: None,
        }
    }

    pub fn parse(&mut self, message: &[u8]) -> Result<(), ParseError> {
        if message.is_empty() {
            return Err(ParseError::EmptyMessage);
        }
        let mut message = message.clone().as_mut();
        self.expect(SYSEX_BEGIN, message)?;
        self.vendor(message)?;
        self.expect(SYSEX_END, message)?;
    }

    pub fn collect(self) -> Vec<AST> {
        self.tokens.drain(..).collect()
    }

    fn vendor(&mut self, message: &mut [u8]) -> Result<(), ParseError> {
        for v in schema::VENDORS.values() {
            if self.accept(&v.sysex, message) {
                let dev =
                self.roots.push(
                    AST::Node(
                        Token::Vendor(v),
                        vec![self.device(v, message)?]
                    ));
                return Ok(())
            }
        }
        Err(ParseError::UnknownVendor)
    }

    fn device(&mut self, vendor: &'static schema::Vendor, message: &mut [u8]) -> Result<AST, ParseError> {
        for d in &vendor.devices {
            if self.accept(&d.sysex, message) {
                let sysex_id = self.next_byte(message)?;
                let _msg_id = self.next_byte(message)?;
                let _unknown = self.next_byte(message)?; // 01 for regular param, 23 for sequences

                return Ok(AST::Chain(
                    Token::Device(d, sysex_id),
                    Box::new(self.control(d, message)?),
                ));
            }
        }
        Err(ParseError::UnknownDevice)
    }

    fn control(&mut self, device: &'static schema::Device, message: &mut [u8]) -> Result<AST, ParseError> {
        if let Some(controls) = &device.controls {
            for c in controls {
                if self.accept(&c.sysex, message) {
                    return Ok(AST::Chain(
                        Token::Control(c),
                        self.bounds(&c.bounds, message)?,
                    ));
                }
            }
        }
        if let Some(controls) = &device.indexed_controls {
            for c in controls {
                if self.accept(&c.sysex, message) {
                    // could decompose into index() if other tokens need it e.g. device
                    let index = self.next_byte(message)?;
                    return Ok(AST::Chain(
                        Token::IndexedControl(c, index),
                        self.bounds(&c.bounds, message)?,
                    ));
                }
            }
        }

        // TODO indexed modal controls

        Err(ParseError::UnknownControl)
    }

    fn bounds(&mut self, bounds: &'static [schema::Bounds], message: &mut [u8]) -> Result<AST, ParseError> {
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => self.values(values, message),
                schema::Bounds::Range(range) => self.in_range(range, message),
                schema::Bounds::MidiNotes(seq) => {
                    let start_offset = self.next_byte(message)?;
                    let seq_length = self.next_byte(message)? as usize;
                    self.note_seq(start_offset, seq_length, seq, message)
                },
            };
            if let Some(token) = check {
                return Ok(AST::Leaf(token))
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

    fn note_seq(&mut self, start_offset: u8, seq_length: usize, range: &'static schema::MidiNotes, message: &mut [u8]) -> Option<Token> {
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

pub fn parse_query(device: &str, items: &mut [String]) -> Result<AST, ParseError> {
    if items.is_empty() {
        return Err(ParseError::EmptyQuery)
    }
    let mut query = TextQuery::new();
    Ok(query.device(device, items)?)
//    Ok(query.tokens.drain(..).collect())
}

#[derive(Debug)]
struct TextQuery {
//    tokens: Vec<Token>,
//    mode: Option<&'static schema::Value>
}

const DIGITS: &str = "0123456789";
const WHITESPACE: &str = " \t";

impl TextQuery {
    pub fn new() -> Self {
        TextQuery {
//            tokens: Vec::with_capacity(6),
//            mode: None,
        }
    }

    fn device(&mut self, device: &str, items: &mut [String]) -> Result<AST,  ParseError> {
        if let Some((vendor, dev)) = schema::DEVICES.get(device) {
            Ok(AST::Node(Token::Vendor(vendor),
                   vec![AST::Node(
                       Token::Device(dev, 1),
                       vec![self.control(dev, items)?]),
                   ])
            )
        } else {
            ParseError::UnknownDevice
        }
    }

    fn control(&mut self, device: &'static schema::Device, items: &mut [String]) -> Result<AST,  ParseError> {
        let (citem, mut items) = items.split_first_mut().ok_or(Err(ParseError::MissingControl))?;
        let seq_parts: Vec<&str> = citem.split("/").collect();
        let cname = seq_parts.get(0).ok_or(Err(ParseError::MissingControlName))?;
        let mut mode_parts: Vec<&str> = citem.split(":").collect();
        match (seq_parts.len(), mode_parts.len()) {
            (1, 1) => {
                let control = device.controls.find(|c| c.name.eq(cname)).ok_or(ParseError::UnknownControl)?;
                Ok(AST::Node(
                    Token::Control(control),
                    vec![self.bounds(&control.bounds, items)?]
                ))
            },
            (2, 1) => {
                let controls = device.indexed_controls.ok_or(ParseError::UnknownControl)?;
                let control = controls.find(|c| c.name.eq(cname)).ok_or(ParseError::UnknownControl)?;
                let idx = u8::from_str(seq_parts.get(1).unwrap()).map_err(|err| Err(ParseError::BadControlIndex))?;
                Ok(AST::Node(
                    Token::IndexedControl(control, idx),
                    vec![self.bounds(&control.bounds, items)?]
                ))
            },
            // TODO
//            (1, 2) => modal control
//            (2, 2) => modal indexed control
            _ => Err(ParseError::BadControlSyntax)
        }
    }

    fn bounds(&mut self, bounds: &'static [schema::Bounds], items: &mut [String]) -> Result<AST,  ParseError> {
        let (value, mut items) = items.split_first_mut().ok_or(Err(ParseError::MissingValue))?;
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => self.values(values, value),
                schema::Bounds::Range(range) => self.in_range(range, value),
                schema::Bounds::MidiNotes(seq) => self.note_seq(seq, value),
            };
            if let Some(token) = check {
                return Ok(AST::Leaf(
                    token,
                ))
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
    fn take(&mut self, matching: &str, input: &mut str) -> Result<String, ParseError> {
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
//    fn expect(&mut self, ident: &str, items: &mut [String]) -> Result<AST,  ParseError> {
//        if self.accept(ident, input) {
//            Ok(())
//        } else {
//            Err(ParseError::Unexpected)
//        }
//    }
}
