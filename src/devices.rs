use std::iter::Iterator;
use linked_hash_map::LinkedHashMap;
use midir::{MidiOutput, MidiInput};

use crate::devices::ParameterBounds::*;

pub type MidiPort = usize;
pub type MidiValue = u8;

pub struct Device {
    usb_vendor_id: u32,
    usb_product_id: u32,
    sysex_out_id: u8,
    sysex_cmd_id: u8,
    name: &'static str,
    params: Vec<Param>,
}

pub struct Param {
    sysex_out_id: u8,
    sysex_cmd_id: u8,
    name: &'static str,
    bounds: ParameterBounds,
}

#[derive(Debug)]
pub enum ParameterBounds {
    Discrete(Vec<(MidiValue, &'static str)>),
    Range(MidiValue, MidiValue),
}

lazy_static!{
    pub static ref DEVICES: Vec<Device> = vec![microbrute()];
    pub static ref PORT_NAMES: LinkedHashMap<&'static str, &'static Device> = port_names();
}

fn port_names() -> LinkedHashMap<&'static str, &'static Device> {
    let mut map = LinkedHashMap::new();
    for dev in DEVICES {
        map.insert(dev.name, &dev);
    }
//    vec![MICROBRUTE].iter().collect()map(|dev| (dev.name, dev)).collect()
    map
}

fn microbrute() -> Device {
    Device {
        name: "Microbrute",
        usb_vendor_id: 0x1c75,
        usb_product_id: 0x0206,
        sysex_out_id: 0x05,
        sysex_cmd_id: 0x06,
        params: vec![
            Param {
                name: "NotePriority",
                sysex_out_id: 0x0b,
                sysex_cmd_id: 0x0c,
                bounds:  Discrete(vec![(0, "LastNote"), (1, "LowNote"), (2, "HighNote")])
            },
            Param {
                name: "VelocityResponse",
                sysex_out_id: 0x10,
                sysex_cmd_id: 0x11,
                bounds:  Discrete(vec![(0, "Logarithmic"), (1, "Exponential"), (2, "Linear")])
            },
            Param {
                name: "Play",
                sysex_out_id: 0x2e,
                sysex_cmd_id: 0x2f,
                bounds:  Discrete(vec![(0, "Hold"), (1, "Note On")])
            },
            Param {
                name: "SeqRetrig",
                sysex_out_id: 0x34,
                sysex_cmd_id: 0x35,
                bounds:  Discrete(vec![(0, "Reset"), (1, "Legato"), (2, "None")])
            },
            Param {
                name: "NextSeq",
                sysex_out_id: 0x32,
                sysex_cmd_id: 0x33,
                bounds: Discrete(vec![(0, "End"), (1, "Reset"), (2, "Continue")])
            },
            Param {
                name: "StepOn",
                sysex_out_id: 0x2a,
                sysex_cmd_id: 0x2b,
                bounds: Discrete(vec![(0, "Clock"), (1, "Gate")])
            },
            Param {
                name: "Step",
                sysex_out_id: 0x38,
                sysex_cmd_id: 0x39,
                // TODO possible custom step hack?
                bounds: Discrete(vec![(0x04, "1/4"), (0x08, "1/8"), (0x10, "1/16"), (0x20, "1/32")])
            },
            Param {
                name: "LfoKeyRetrig",
                sysex_out_id: 0x0f,
                sysex_cmd_id: 0x10,
                bounds: Discrete(vec![(0, "Off"), (1, "On")])
            },
            Param {
                name: "EnvLegatoMode",
                sysex_out_id: 0x0d,
                sysex_cmd_id: 0x0e,
                bounds: Discrete(vec![(0, "Off"), (1, "On")])
            },
            Param {
                name: "Gate",
                sysex_out_id: 0x36,
                sysex_cmd_id: 0x37,
                bounds: Discrete(vec![(1, "Short"), (2, "Medium"), (3, "Long")])
            },
            Param {
                name: "Sync",
                sysex_out_id: 0x3c,
                sysex_cmd_id: 0x3d,
                bounds: Discrete(vec![(0, "Auto"), (1, "Internal"), (2, "External")])
            },
            Param {
                name: "BendRange",
                sysex_out_id: 0x2c,
                sysex_cmd_id: 0x2d,
                bounds: Range(1, 12)
            },
            Param {
                name: "MidiRecvChan",
                sysex_out_id: 0x05,
                sysex_cmd_id: 0x06,
                bounds: Range(1, 16)
            },
            Param {
                name: "MidiSendChan",
                sysex_out_id: 0x07,
                sysex_cmd_id: 0x08,
                bounds: Range(1, 16)
            },
        ]
    }
}

pub fn output_ports(midi: &MidiOutput) -> Vec<(MidiPort, &'static Device)> {
    (0..midi.port_count())
        .filter_map(|idx| midi.port_name(idx).map(|name| (idx, name)).ok())
        .filter_map(|(idx, name)| PORT_NAMES.get(name.as_str()).map(|dev| (idx, dev)))
        .collect()
}

pub fn input_ports(midi: &MidiInput) -> Vec<(MidiPort, &'static Device)> {
    (0..midi.port_count())
        .filter_map(|idx| midi.port_name(idx).map(|name| (idx, name)).ok())
        .filter_map(|(idx, name)| PORT_NAMES.get(name.as_str()).map(|dev| (idx, dev)))
        .collect()
}

//fn main() -> core::Result<()> {
    // open midi "ports"
//    let midi_out = MidiOutput::new("LaBruteforce")?;
//    let out_port =
//        lookup_out_port(&midi_out, microbrute::PORT_NAME).ok_or("No Microbrute Out Port")?;
//    let conn_out = midi_out.connect(out_port, "Microbrute Control")?;

//    match Microbrute::from_midi(conn_out) {
//        Ok(brute) => {
//            let mut raw_term = stdout().into_raw_mode()?;
//            write!(raw_term, "{}", termion::cursor::Hide)?;
//            raw_term.flush()?;
//
//            let mut ui = ui::ParamMenu::new(Box::new(brute), None);
//
//            ui.run(&mut raw_term)?;
//            println!("{}{}", style::Reset, cursor::Left(0))
//        }
//        Err(err) => println!("{}", err)
//    }
//    Ok(())
//}

//fn known_devices(midi_out: &MidiOutput, midi_in: &MidiInput) -> core::Result<Vec<Microbrute>> {
//    // enumerate devices, detected and configured
//    // find single microbrute for now
////    let out_port = midi::enum_out_port(midi_out);
//    let out_port = midi::lookup_out_port(&midi_out, &microbrute::dev_name()).ok_or("No Microbrute Out Port")?;
//    let in_port = midi::lookup_in_port(&midi_in, &microbrute::dev_name()).ok_or("No Microbrute In Port")?;
//
//    let device = Microbrute::default();
//    Ok(vec![])
//
//}
