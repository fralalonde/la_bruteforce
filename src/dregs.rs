
pub struct ParamKey {
    pub name: String,
    pub param: Parameter,
    pub index: Option<usize>,
    pub mode: Option<Mode>,
}

impl fmt::Display for ParamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some(index) = &self.index {
            f.write_fmt(format_args!("/{}", index))?;
        }
        if let Some(mode) = &self.mode {
            f.write_fmt(format_args!(":{}", mode))?;
        }
        Ok(())
    }
}

pub fn parse_key(&self, param_key: &str) -> Result<ParamKey> {
    let seq_parts: Vec<&str> = param_key.split("/").collect();
    let name: &str = seq_parts.get(0).ok_or("Empty param key")?;
    let mut mode_parts: Vec<&str> = param_key.split(":").collect();
    let (index, mode) =  match (seq_parts.len(), mode_parts.len()) {
        (1, 1) => (None, None),
        (2, 1) => (seq_parts.get(1), None),
        (1, 2) => (None, mode_parts.get(1)),
        (2, 2) => {
            // i.e. "Seq/3:Mode" : re-split "3" from "Mode"
            mode_parts = seq_parts.get(1).unwrap().split(":").collect();
            (mode_parts.get(0), mode_parts.get(1))
        },
        _ => Err(DeviceError::UnknownParameter {
            param_name: param_key.to_string(),
        })?
    };
    let param = self
        .parameters
        .get(name)
        .ok_or(DeviceError::UnknownParameter {
            param_name: param_key.to_string(),
        })?;

    let index_val = if let Some(idx_match) = index {
        Some(usize::from_str(*idx_match)?)
    } else {
        None
    };
    let index = match (index_val, param.range) {
        (Some(value), Some(range)) if value >= range.lo && value <= range.hi => Some(value),
        (None, None) => None,
        _ => {
            return Err(Box::new(DeviceError::BadIndexParameter {
                param_name: param_key.to_string(),
            }))
        }
    };

    let mode = match (mode, &param.modes) {
        (Some(mode_str), Some(modes)) => {
            if let Some(mode) = modes.get(*mode_str) {
                Some(mode.clone())
            } else {
                return Err(Box::new(DeviceError::BadModeParameter {
                    param_name: param_key.to_string(),
                }));
            }
        }
        (None, None) => None,
        _ => {
            return Err(Box::new(DeviceError::BadModeParameter {
                param_name: param_key.to_string(),
            }))
        }
    };

    Ok(ParamKey {
        param: param.clone(),
        name: name.to_string(),
        index,
        mode,
    })
}


impl ParamKey {
    pub fn bounds(&self, field_name: Option<&str>) -> Result<Vec<Bounds>> {
        match (self, field_name) {
            (
                ParamKey {
                    mode: Some(mode), ..
                },
                Some(field_name),
            ) => Ok(mode
                .fields
                .get(field_name)
                .ok_or(DeviceError::BadField {
                    field_name: field_name.to_string(),
                })?
                .bounds
                .clone()),
            (_, None) => Ok(self.param.bounds.clone().ok_or(DeviceError::BadSchema {
                field_name: self.name.clone(),
            })?),
            _ => Err(Box::new(DeviceError::NoBounds)),
        }
    }

    fn fields(&self) -> Option<LinkedHashMap<String, Field>> {
        self.mode.clone().map(|mode| mode.fields)
    }

    pub fn parse_value(&self, value: &str) -> Result<Value> {
        let parts: Vec<&str> = value.split("=").collect();
        match (parts.as_slice(), &self.fields()) {
            ([field_name, value], Some(fields)) => {
                for b in self.bounds(Some(field_name))? {
                    if let Ok(v) = b.convert(value) {
                        return Ok(Value::FieldValue(field_name.to_string(), v));
                    }
                }
            }
            ([value], None) => {
                for b in self.bounds(None)? {
                    if let Ok(v) = b.convert(value) {
                        return Ok(Value::ParamValue(v));
                    }
                }
            }
            _ => {}
        }
        Err(Box::new(DeviceError::ValueOutOfBound {
            value_name: value.to_string(),
        }))
    }
}

pub enum Value {
    ParamValue(Vec<u8>),
    FieldValue(String, Vec<u8>),
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for z in &self.fields {
            f.write_fmt(format_args!("{}", z.0))?;
        }
        Ok(f.write_str("}")?)
    }
}

impl Bounds {
    pub fn convert(&self, value: &str) -> Result<Vec<u8>> {
        match self {
            Bounds::Values(values) => Ok(vec![*values.get(value).ok_or_else(|| {
                DeviceError::UnknownValue {
                    value_name: value.to_owned(),
                }
            })?]),
            Bounds::Range(range) => {
                let val = usize::from_str(value)?;
                if val >= range.lo && val <= range.hi {
                    Ok(vec![if let Some(offset) = range.offset {
                        (val - offset) as u8
                    } else {
                        val as u8
                    }])
                } else {
                    Err(Box::new(DeviceError::ValueOutOfBound {
                        value_name: value.to_owned(),
                    }))
                }
            }
            Bounds::MidiNotes(noteseq) => {
                let offset = noteseq.offset.unwrap_or(0);
                let mut notes = Vec::with_capacity(noteseq.max_len);
                for v in value.split(",") {
                    notes.push((MidiNote::from_str(v)?.note as i8 + offset) as u8)
                }
                Ok(notes)
            }
        }
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


fn decode(schema: schema::Device, msg: &[u8], result_map: &mut LinkedHashMap<String, Vec<String>>) {
    let param = schema.parse_msg(msg);
    if let Some(param) = into_param(msg) {
        match param {
            NoteSeq(_idx) => {
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
                if let Some(bound) = devicf::bound_str(bounds(param), &[msg[4]]) {
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

#[derive(Debug, Clone)]
pub enum Bounds {
    /// List of raw value and display name pairs
    Discrete(Vec<(MidiValue, &'static str)>),

    /// Raw value offset and display value bounds (Low to High, inclusive)
    Range(u8, (MidiValue, MidiValue)),

    /// Sequence of notes with offset from std MIDI note value
    NoteSeq(u8),
}


pub fn bound_codes(bounds: Bounds, bound_ids: &[String], reqs: (usize, usize)) -> Result<Vec<u8>> {
    if bound_ids.len() < reqs.0 {
        return Err(Box::new(DeviceError::MissingValue {
            param_name: "param".to_string(),
        }));
    }
    if bound_ids.len() > reqs.1 {
        return Err(Box::new(DeviceError::TooManyValues {
            param_name: "param".to_string(),
        }));
    }
    match bounds {
        Bounds::Discrete(values) => {
            let b_id = bound_ids.get(0).unwrap();
            for v in &values {
                if v.1.eq(b_id) {
                    return Ok(vec![v.0]);
                }
            }
            Err(Box::new(DeviceError::UnknownValue {
                value_name: b_id.to_owned(),
            }))
        }
        Bounds::Range(offset, (lo, hi)) => {
            let b_id = bound_ids.get(0).unwrap();
            let val = u8::from_str(b_id)?;
            if val >= lo && val <= hi {
                Ok(vec![val - offset])
            } else {
                Err(Box::new(DeviceError::ValueOutOfBound {
                    value_name: b_id.to_owned(),
                }))
            }
        }
        Bounds::NoteSeq(offset) => {
            let mut bcode = Vec::with_capacity(bound_ids.len());
            for b_id in bound_ids {
                bcode.push(MidiNote::from_str(b_id)?.note + offset);
            }
            Ok(bcode)
        }
    }
}

