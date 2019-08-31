use core::{discrete, range, Device, ParamEnum, ParameterDef, Result};
use microbrute::MicrobruteParameter::*;

use midi::{lookup_in_port, MidiValue};
use midir::{MidiInput, MidiOutputConnection};

use linked_hash_map::LinkedHashMap;
use std::collections::HashMap;
use std::fmt;

lazy_static! {
    pub static ref MICROBRUTE: LinkedHashMap<MicrobruteParameter, Box<ParameterDef + Send + std::marker::Sync>> =
        build_param_map();
}

pub const PORT_NAME: &'static str = "MicroBrute";

fn build_param_map() -> LinkedHashMap<MicrobruteParameter, Box<ParameterDef + Send + std::marker::Sync>> {
    let mut map = LinkedHashMap::new();
    map.insert(
        NotePriority,
        discrete(
            "Note Priority",
            Some('n'),
            vec![(0, "Last Note"), (1, "Lowest Note"), (2, "Highest Note")],
        ),
    );
    map.insert(
        VelocityResponse,
        discrete(
            "Velocity Response Curve",
            Some('v'),
            vec![(0, "Logarithmic"), (1, "Exponential"), (2, "Linear")],
        ),
    );
    map.insert(
        Play,
        discrete("Play", Some('p'), vec![(0, "Hold"), (1, "Note On")]),
    );
    map.insert(
        SeqRetrig,
        discrete(
            "Sequencer Retriggering",
            Some('q'),
            vec![(0, "Reset"), (1, "Legato"), (2, "None")],
        ),
    );
    map.insert(
        NextSeq,
        discrete(
            "Next Seq",
            Some('n'),
            vec![(0, "End"), (1, "Instant Reset"), (2, "Instant Continue")],
        ),
    );
    map.insert(
        StepOn,
        discrete("Step On", Some('o'), vec![(0, "Clock"), (1, "Gate")]),
    );
    map.insert(
        Step,
        discrete(
            "Step",
            Some('s'),
            vec![
                // TODO possible custom step hack?
                (0x04, "1/4"),
                (0x08, "1/8"),
                (0x10, "1/16"),
                (0x20, "1/32"),
            ],
        ),
    );
    map.insert(
        LfoKeyRetrig,
        discrete("LFO Key Retrigger", Some('l'), vec![(0, "Off"), (1, "On")]),
    );
    map.insert(
        EnvLegatoMode,
        discrete("Envelope Legato Mode", Some('d'), vec![(0, "Off"), (1, "On")]),
    );
    map.insert(
        Gate,
        discrete(
            "Gate",
            Some('g'),
            vec![
                // NOTE starts at 1
                (1, "Short"),
                (2, "Medium"),
                (3, "Long"),
            ],
        ),
    );
    map.insert(
        Sync,
        discrete(
            "Sync",
            Some('y'),
            vec![(0, "Auto"), (1, "Internal"), (2, "External")],
        ),
    );
    map.insert(BendRange, range("Bend Range", Some('b'), (1, 12)));
    map.insert(MidiRecvChan, range("MIDI RX", Some('r'), (1, 16)));
    map.insert(MidiSendChan, range("MIDI TX", Some('t'), (1, 16)));

    map
}

pub struct Microbrute {
    conn_out: MidiOutputConnection,
    pub sysex_counter: usize,
    state: HashMap<MicrobruteParameter, MidiValue>,
}

impl fmt::Debug for Microbrute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(sysex_counter {}, state {:?})",
            self.sysex_counter, self.state
        )
    }
}

fn is_microbrute_sysex(message: &[u8]) -> bool {
    message[1] == 0x00 &&
        message[2] == 0x20 &&
        message[3] == 0x6b &&
        message[4] == 0x05 && // Microbrute
        message[5] == 0x01 &&
        message[7] == 0x01
}

impl Microbrute {
    pub fn from_midi(mut conn_out: MidiOutputConnection) -> Result<Self> {
        // setup listener first
        let midi_in = MidiInput::new("La_BruteForce In")?;
        let in_port = lookup_in_port(&midi_in, PORT_NAME).ok_or("No Microbrute Connected")?;
        let conn_in = midi_in.connect(
            in_port,
            "Microbrute In",
            |ts, message, state_map| handle_midi_response(state_map, ts, message),
            HashMap::<MicrobruteParameter, MidiValue>::new(),
        )?;

        let sysex_counter = request_state(&mut conn_out, 0)?;
        std::thread::sleep_ms(250);
        let state = conn_in.close().1;

        Ok(Microbrute {
            conn_out,
            sysex_counter,
            state,
        })
    }
}

impl Device for Microbrute {
    type PARAM = MicrobruteParameter;

    fn parameters(
        &self,
    ) -> &'static LinkedHashMap<Self::PARAM, Box<ParameterDef + Send + std::marker::Sync>> {
        &MICROBRUTE
    }

    fn get(&self, param: Self::PARAM) -> Option<MidiValue> {
        self.state.get(&param).map(|x| *x)
    }

    fn set(&mut self, param: Self::PARAM, value: MidiValue) -> Result<Option<MidiValue>> {
        self.conn_out
            .send(&sysex_update_param(self.sysex_counter, param, value))?;
        self.sysex_counter += 1;
        Ok(self.state.insert(param, value))
    }
}

fn request_state(conn_out: &mut MidiOutputConnection, mut sysex_count: usize) -> Result<usize> {
    conn_out.send(&SYSEX_QUERY_START)?;
    for param_id in &SYSEX_PARAM_IDS {
        conn_out.send(&sysex_query_param(sysex_count, *param_id))?;
        sysex_count += 1;
    }
    Ok(sysex_count)
}

fn handle_midi_response(state_map: &mut HashMap<MicrobruteParameter, MidiValue>, _ts: u64, message: &[u8]) {
    let len = message.len();
    // is sysex message?
    if message[0] == 0xf0 && message[len - 1] == 0xf7 {
        if is_microbrute_sysex(message) {
            let code = message[8];
            match MicrobruteParameter::from_midi(code) {
                Some(param) => {
                    let value = message[9] as MidiValue;
                    state_map.insert(param, value);
                    // TODO update UI if not UI queried
                },
                None => {} // eprintln!("unknown parameter '{}'", code),
            }
        }
    }
}

param_enum!( MicrobruteParameter {
    NotePriority => 0x0b,
    VelocityResponse => 0x11,
    Play => 0x2e,
    SeqRetrig => 0x34,
    NextSeq => 0x32,
    StepOn => 0x2a,
    Step => 0x38,
    LfoKeyRetrig => 0x0f,
    EnvLegatoMode => 0x0d,
    Gate => 0x36,
    Sync => 0x3c,
    BendRange => 0x2c,
    MidiRecvChan => 0x05,
    MidiSendChan => 0x07,
});

const SYSEX_QUERY_START: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];

const SYSEX_PARAM_IDS: [u8; 14] = [
    0x06, 0x08, 0x35, 0x10, 0x2f, 0x0c, 0x0e, 0x12, 0x33, 0x2d, 0x39, 0x37, 0x2b, 0x3d,
];

fn sysex_query_param(counter: usize, datum: u8) -> [u8; 10] {
    [
        0xf0,
        0x00,
        0x20,
        0x6b,
        0x05,
        0x01,
        counter as u8,
        0x00,
        datum,
        0xf7,
    ]
}

fn sysex_update_param(counter: usize, param: MicrobruteParameter, value: MidiValue) -> [u8; 11] {
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
