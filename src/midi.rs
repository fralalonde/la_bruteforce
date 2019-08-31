use midir::{MidiInput, MidiOutput};

pub type MidiPort = usize;

pub type MidiValue = u8;

pub fn lookup_out_port(midi_out: &MidiOutput, name: &str) -> Option<MidiPort> {
    for i in 0..midi_out.port_count() {
        if midi_out.port_name(i).unwrap().starts_with(name) {
            return Some(i as MidiPort);
        }
    }
    None
}

pub fn lookup_in_port(midi_in: &MidiInput, name: &str) -> Option<MidiPort> {
    for i in 0..midi_in.port_count() {
        if midi_in.port_name(i).unwrap().starts_with(name) {
            return Some(i as MidiPort);
        }
    }
    None
}

//pub fn enum_out_port(midi_out: &MidiOutput) -> HashMap<MidiPort, String> {
//    (0..midi_out.port_count())
//        .filter_map(|port_num| (port_num, midi_out.port_name(port_num)
//            .unwrap_or_else(|err| {
//                eprintln!("Could not get port name: {}", err);
//                None
//            })))
//        .collect()
//}
//
//pub fn enum_in_port(midi_in: &MidiInput) -> Vec<(MidiPort, String)> {
//    (0..midi_in.port_count())
//        .map(|port_num| (port_num, midi_in.port_name(port_num).unwrap()))
//        .collect()
//}
