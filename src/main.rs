extern crate midir;
#[macro_use]
extern crate lazy_static;
extern crate linked_hash_map;

extern crate termion;

#[macro_use]
mod core;
mod microbrute;
mod midi;
mod ui;

use microbrute::Microbrute;
use midir::{MidiOutput};

use termion::raw::{IntoRawMode};
use termion::{cursor, style};

use std::io::{stdout, Write};

fn main() -> core::Result<()> {
    // open midi "ports"
    let midi_out = MidiOutput::new("LaBruteforce Out")?;
    let out_port =
        midi::lookup_out_port(&midi_out, microbrute::PORT_NAME).ok_or("No Microbrute Out Port")?;
    let conn_out = midi_out.connect(out_port, "Microbrute Control")?;

    let brute = Microbrute::from_midi(conn_out)?;

    let mut raw_term = stdout().into_raw_mode()?;
    write!(raw_term, "{}", termion::cursor::Hide)?;
    raw_term.flush()?;

    let mut ui = ui::ParamMenu::new(Box::new(brute), None);

    let ui_result = ui.run(&mut raw_term);
    println!("{}{}", style::Reset, cursor::Left(0));
    ui_result
}

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
