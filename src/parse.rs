use crate::devices::{DeviceError};
use crate::parse::Token::{Value, Control, Vendor};
use crate::{schema, parse};
use std::collections::VecDeque;
use snafu::*;
use std::str::FromStr;
use std::num::ParseIntError;
use crate::schema::MidiNote;
use std::ops::Deref;
use std::cell::RefCell;
use indextree::{Arena, NodeId, Node};

type Result<T> = ::std::result::Result<T, ParseError>;

#[derive(Debug, Snafu)]
pub enum ParseError {
    Unexpected,
    UnknownVendor,
    UnknownDevice,
    UnknownControl {
        text: String
    },
    NoMatchingBounds,
    ShortRead,
    EmptyMessage,
    EmptyQuery,
    MissingDevice,
    MissingControl,
    MissingControlName,
    BadControlSyntax{
        text: String
    },
    BadControlIndex,
    MissingValue,
    BadNoteSyntax,
    ExtraneousChars,
}


#[derive(Debug, Clone)]
pub enum Token {
    /// Root of AST
    Sysex,

    Vendor(&'static schema::Vendor),
    Device(&'static schema::Device, u8),

//    not correlating replies, assuming they'll be ordered enough
//    ReplyId(u8),

//    Patch(usize),

    Control(&'static schema::Control),
    IndexedControl(&'static schema::IndexedControl, u8),

    Mode(&'static schema::Value),
    Field(&'static schema::Field),

    Value(&'static schema::Value),
    InRange(&'static schema::Range, isize),
    MidiNotes(&'static schema::MidiNotes, u8, Vec<MidiNote>),
}

#[derive(Debug, Default, Clone)]
struct Buffer {
    head: Vec<u8>,
    /// careful: tail bytes are in reverse order
    tail: Vec<u8>,
}

impl Into<Vec<u8>> for Buffer {
    fn into(mut self) -> Vec<u8> {
        self.tail.reverse();
        self.head.extend_from_slice(&self.tail);
        self.head
    }
}

const SYSEX_BEGIN: &[u8] = &[0xf0];
const SYSEX_END: &[u8] = &[0xf7];

impl Token {
    pub fn to_sysex(&self, buffer: &mut Buffer) {
        match self {
            Token::Sysex => {
                buffer.head.extend_from_slice(SYSEX_BEGIN);
                buffer.tail.extend_from_slice(SYSEX_END);
            }

            Token::Vendor(v) => buffer.head.extend_from_slice(&v.sysex),
            Token::Device(d, idx) => {
                buffer.head.extend_from_slice(&d.sysex);
                buffer.head.push(*idx);
            },
            Token::Control(c) => buffer.head.extend_from_slice(&c.sysex),
            Token::IndexedControl(c, idx) => {
                buffer.head.extend_from_slice(&c.sysex);
                buffer.head.push(*idx);
            },

            Token::Mode(m) => buffer.head.push(m.sysex),
            Token::Field(f) => buffer.head.extend_from_slice(&f.sysex),

            Token::Value(v) => buffer.head.push(v.sysex),
            Token::InRange(r, idx) => buffer.head.push((*idx) as u8),
            Token::MidiNotes(s, offset, notes) => {
                buffer.head.push(*offset);
                buffer.head.push(notes.len() as u8);
                buffer.head.extend(notes.iter().map(|note| *note.deref()))
            },
        }
    }
}

impl  AST {

    /// Depth-first walk
    pub fn find_map<Z, F: Fn(&Token) -> Option<Z>>(&self, op: &F) -> Option<Z> {
        self.walk_find_map(self.root, op)
    }

    /// Depth-first walk
    fn walk_find_map<Z, F: Fn(&Token) -> Option<Z>>(&self, node: NodeId, op: &F) -> Option<Z> {
        if let Some(z) = op(self.arena[node].get()) {
            return Some(z)
        }
        for c in node.children(&self.arena) {
            if let Some(z) = self.walk_find_map(c, op) {
                return Some(z)
            }
        }
        None
    }

    pub fn to_sysex(&self, msg_id: &mut usize) -> Result<Vec<Vec<u8>>> {
        let mut messages: Vec<Vec<u8>> = vec![];
        let mut buffer = Buffer::default();
        self.to_sysex_inner(self.root, buffer, &mut messages);
        Ok(messages)
    }

    fn to_sysex_inner(&self, node_id: NodeId, mut buffer: Buffer, messages: &mut Vec<Vec<u8>>) {
        let node: &Node<Token> = &self.arena[node_id];
        node.get().to_sysex(&mut buffer);
        if let Some(first_child) = node.first_child() {
            if Some(first_child) == node.last_child() {
                // only child, no need to clone & fork
                self.to_sysex_inner(first_child, buffer, messages);
            } else {
                for c in node_id.children(&self.arena) {
                    self.to_sysex_inner(c, buffer.clone(), messages);
                }
            }
        } else {
            messages.push(buffer.into())
        }
    }
}

#[derive(Debug)]
pub struct AST {
    arena: Arena<Token>,
    root: NodeId,
}

impl AST {
    fn new() -> Self {
        let mut arena = Arena::new();
        AST {
            root: arena.new_node(Token::Sysex),
            arena,
        }
    }

    fn push_child(&mut self, node: NodeId, token: Token) -> NodeId {
        let child_node = self.arena.new_node(token);
        node.append(child_node, &mut self.arena);
        child_node
    }
}

#[derive(Debug)]
pub struct SysexReply {
    ast: AST,
    mode: Option<&'static schema::Value>,
}

impl  SysexReply {
    pub fn new() -> Self {
        SysexReply {
            ast: AST::new(),
            mode: None,
        }
    }

    pub fn parse(&mut self, message: &[u8]) -> Result<()> {
        if message.is_empty() {
            return Err(ParseError::EmptyMessage);
        }
        // TODO find a way to not copy the message
        let message = &mut message.to_vec();
        self.expect(SYSEX_BEGIN, message)?;
        self.vendor(self.ast.root, message)?;
        // SYSEX_END expected by leaf nodes
        Ok(())
    }

    pub fn collect(mut self) -> AST {
        self.ast
    }

    fn vendor(&mut self, node: NodeId, message: &mut [u8]) -> Result<()> {
        for v_schema in schema::VENDORS.values() {
            if self.accept(&v_schema.sysex, message) {
                let v_node = self.ast.push_child(node, Token::Vendor(v_schema));
                return self.device(v_node, v_schema, message);
            }
        }
        Err(ParseError::UnknownVendor)
    }

    fn device(&mut self, node: NodeId, vendor: &'static schema::Vendor, message: &mut [u8]) -> Result<()> {
        for d_schema in &vendor.devices {
            if self.accept(&d_schema.sysex, message) {
                let sysex_id = self.next_byte(message)?;
                let _reply_id = self.next_byte(message)?;
                let _unknown = self.next_byte(message)?; // 01 for regular param, 23 for sequences
                let d_node = self.ast.push_child(node, Token::Device(d_schema, sysex_id));
                return self.control(d_node, d_schema, message);
            }
        }
        Err(ParseError::UnknownDevice)
    }

    fn control(&mut self, node: NodeId, device: &'static schema::Device, message: &mut [u8]) -> Result<()> {
        if let Some(controls) = &device.controls {
            for c_schema in controls {
                if self.accept(&c_schema.sysex, message) {
                    let c_node = self.ast.push_child(node, Token::Control(c_schema));
                    return self.bounds(c_node, &c_schema.bounds, message);
                }
            }
        }
        if let Some(controls) = &device.indexed_controls {
            for ic_schema in controls {
                if self.accept(&ic_schema.sysex, message) {
                    // could decompose into index() if other tokens need it e.g. device
                    let index = self.next_byte(message)?;
                    let ic_node = self.ast.push_child(node, Token::IndexedControl(ic_schema, index));
                    return self.bounds(ic_node, &ic_schema.bounds, message);
                }
            }
        }

        // TODO indexed modal controls

        Err(ParseError::UnknownControl{text: hex::encode(message)})
    }

    fn bounds(&mut self, node: NodeId, bounds: &'static [schema::Bounds], message: &mut [u8]) -> Result<()> {
        for b_schema in bounds {
            let check = match b_schema {
                schema::Bounds::Values(values) => self.values(values, message),
                schema::Bounds::Range(range) => self.in_range(range, message),
                schema::Bounds::MidiNotes(seq) => {
                    let start_offset = self.next_byte(message)?;
                    let seq_length = self.next_byte(message)? as usize;
                    self.note_seq(start_offset, seq_length, seq, message)
                },
            };
            if let Some(token) = check {
                let ic_node = self.ast.push_child(node, token);
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

    fn take(&mut self, length: usize, message: &mut [u8]) -> Result<Vec<u8>> {
        if message.is_empty() {
            return Err(ParseError::ShortRead)
        };
        let (a, _message) = message.split_at_mut(length);
        Ok(a.to_vec())
    }

    fn next_byte(&mut self, message: &mut [u8]) -> Result<u8> {
        let (z, _message) = message.split_first_mut().ok_or(ParseError::ShortRead)?;
        Ok(*z)
    }


    fn expect(&mut self, value: &[u8], message: &mut [u8]) -> Result<()> {
        if self.accept(value, message) {
            Ok(())
        } else {
            Err(ParseError::Unexpected)
        }
    }
}

pub fn parse_query(device: &str, items: &mut [String]) -> Result<AST> {
    if items.is_empty() {
        return Err(ParseError::EmptyQuery)
    }
    let mut parser = TextParser::new(false);
    parser.device(parser.ast.root, device, items)?;
    Ok(parser.ast)
}

pub fn parse_update(device: &str, items: &mut [String]) -> Result<AST> {
    if items.is_empty() {
        return Err(ParseError::EmptyQuery)
    }
    let mut parser = TextParser::new(true);
    parser.device(parser.ast.root, device, items)?;
    Ok(parser.ast)
}

#[derive(Debug)]
struct TextParser {
    ast: AST,
    mode: Option<&'static schema::Value>,
    for_update: bool,
}

const DIGITS: &str = "0123456789";
const WHITESPACE: &str = " \t";

impl  TextParser {

    fn new(for_update: bool) -> Self {
        TextParser {
            ast: AST::new(),
            mode: None,
            for_update,
        }
    }

    fn device(&mut self, node: NodeId, device: &str, items: &mut [String]) -> Result<()> {
        if let Some((vendor, dev)) = schema::DEVICES.get(device) {
            let v_node = self.ast.push_child(node, Token::Vendor(vendor));
            let d_node = self.ast.push_child(v_node, Token::Device(dev, 1));
            self.control(d_node, dev, items)
        } else {
            Err(ParseError::UnknownDevice)
        }
    }

    fn control(&mut self, node: NodeId, device: &'static schema::Device, items: &mut [String]) -> Result<()> {
        let (citem, mut items) = items.split_first_mut().ok_or(ParseError::MissingControl)?;
        let seq_parts: Vec<&str> = citem.split("/").collect();
        let cname = seq_parts.get(0).ok_or(ParseError::MissingControlName)?;
        let mut mode_parts: Vec<&str> = citem.split(":").collect();
        let (ctoken, bounds) = match (seq_parts.len(), mode_parts.len()) {
            (1, 1) => {
                let control = device.controls.iter().flatten()
                    .find(|c| c.name.eq(cname))
                    .ok_or(ParseError::UnknownControl{text: cname.to_string()})?;
                Ok((Token::Control(control), &control.bounds))
            },
            (2, 1) => {
                let control = device.indexed_controls.iter().flatten()
                    .find(|c| c.name.eq(cname))
                    .ok_or(ParseError::UnknownControl{text: cname.to_string()})?;
                let idx = u8::from_str(seq_parts.get(1).unwrap()).map_err(|err| ParseError::BadControlIndex)?;
                Ok((Token::IndexedControl(control, idx), &control.bounds))
            },
            // TODO
//            (1, 2) => modal control
//            (2, 2) => modal indexed control
            _ => Err(ParseError::BadControlSyntax{text: cname.to_string()})
        }?;

        let d_node = self.ast.push_child(node, ctoken);

        if self.for_update {
            self.bounds(d_node, &bounds, items)
        } else if items.is_empty() {
            Ok(())
        } else {
            Err(ParseError::ExtraneousChars)
        }
    }

    fn bounds(&mut self, node: NodeId, bounds: &'static [schema::Bounds], items: &mut [String]) -> Result<()> {
        let (value, mut _items) = items.split_first_mut().ok_or(ParseError::MissingValue)?;
        for b in bounds {
            let check = match b {
                schema::Bounds::Values(values) => self.values(values, value),
                schema::Bounds::Range(range) => self.in_range(range, value),
                schema::Bounds::MidiNotes(seq) => self.note_seq(seq, value),
            };
            if let Some(token) = check {
                self.ast.push_child(node, token);
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
    fn take(&mut self, matching: &str, input: &mut str) -> Result<String> {
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
//    fn expect(&mut self, ident: &str, items: &mut [String]) -> Result<AST> {
//        if self.accept(ident, input) {
//            Ok(())
//        } else {
//            Err(ParseError::Unexpected)
//        }
//    }
}
