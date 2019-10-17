use strum::ParseError;
use crate::devices::DeviceError;
use crate::parse::Token::{Value, Control, Index, Offset, Length, Sequence, Vendor};
use crate::schema;
use std::collections::VecDeque;

#[derive(Debug, Snafu)]
pub enum ParseError {
    Unexpected {
//        device_name: String,
    },
}

enum Token {
    Vendor(&'static schema::Vendor),
    Device(&'static schema::Device),
    SysexId(usize),
    Patch(usize),

    Control(&'static schema::Control),
    IndexedControl(&'static schema::Control, usize),

    Mode(&'static schema::Value),
    Field(&'static schema::Field),

    Value(&'static schema::Value),
    Range(&'static schema::Range, isize),
    MidiNotes(&'static schema::MidiNotes, usize, Vec<u8>),
}

const SYSEX_BEGIN: &[u8] = &[0xf0];
const SYSEX_END: &[u8] = &[0xf7];

#[derive(Debug)]
struct SysexReply<'a> {
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
            return ParseError::EmptyMessage;
        }
        self.expect(SYSEX_BEGIN)?;
        self.vendor(message)?;
        self.expect(SYSEX_END)?;
        Ok(self.tokens.drain([..]).collect())
    }

    fn vendor(&mut self, message: &[u8]) -> Result<(), ParseError> {
        for v in VENDORS.iter() {
            if accept(v.sysex) {
                self.tokens.push(Token::Vendor(v));
                self.device(v, message)
            }
        }
        Ok(())
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


fn arturia(parse: &mut SysexReply) -> Result<(), ParseError> {
    parse.expect(&[0x01])?; // device sysex id?
    parse.take(1)?; // msg id, unused for now
    parse.take(1)?; // unknown, ignore (01 for regular param, 23 for sequences)
    if parse.accept(MICROBRUTE) {
        microbrute(parse)
    } else {
        Err(ParseError::UnknownDevice)
    }
}

fn microbrute(parse: &mut SysexReply) -> Result<(), ParseError> {
    // 01       sysex id    @5
    // 0f       message id  @6
    // 01       unknown     @7
    // 11       control id  @8
    // 00       value       @9
    // 00 00 00 00 00 00 00 00 padding
    // f7       sysex end   @18
    if parse.accept(SEQUENCE) {
        sequence(parse)
    } else {
        let control = parse.take(1)?;
        parse.tokens.push(Control(control));
        let value = parse.take(1)?;
        parse.tokens.push(Value(value));
    }
}

fn sequence(parse: &mut SysexReply) -> Result<(), ParseError> {
    // 23 3a 00 00 20 30 3c 48 54 26 74 51 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 f7
    
    parse.tokens.push(Index(parse.take(1)?));
    parse.take(1)?;
    parse.tokens.push(Offset(parse.take(1)?));
    parse.tokens.push(Length(parse.take(1)?));
    parse.tokens.push(Sequence(parse.take(32)?));
    Ok(())
}

fn step_on(parse: &mut SysexReply) -> Result<(), ParseError> {
    if let Some(value) = parse.take(1) {
        parse.tokens.push(Value(value))
    } else {
        Err(ParseError::UnknownParameter)
    }
}
