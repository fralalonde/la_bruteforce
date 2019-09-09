use crate::devices::ParameterBounds::*;
use crate::devices::{DeviceDescriptor, Param};
use midir::{MidiOutputConnection, MidiOutput};
use midi::Result;

pub struct MicroBrute {
    midi_connection: MidiOutputConnection,
    sysex_counter: usize,
}

pub trait Device {
    fn descriptor() -> DeviceDescriptor;

    fn new(midi_out: &MidiOutput, port: &MidiPort) -> Result<Self>;
}

pub struct MidiPort {
    number: usize,
    name: String,
}

impl Device for MicroBrute {
    fn new(midi_out: &MidiOutput, port: &MidiPort) -> Result<Self> {
        Ok(MicroBrute {
            midi_connection: midi_out.connect(port.number, &port.name)?,
            sysex_counter: 0,
        })
    }

    fn descriptor() -> DeviceDescriptor {
        DeviceDescriptor {
            name: "MicroBrute",
            port_name_prefix: "MicroBrute",
            usb_vendor_id: 0x1c75,
            usb_product_id: 0x0206,
            sysex_out_id: 0x05,
            sysex_tx_id: 0x06,
            params: vec![
                Param {
                    name: "KeyNotePriority",
                    sysex_rx_id: 0x0b,
                    sysex_tx_id: 0x0c,
                    bounds: Discrete(vec![(0, "LastNote"), (1, "LowNote"), (2, "HighNote")]),
                },
                Param {
                    name: "KeyVelocityResponse",
                    sysex_rx_id: 0x10,
                    sysex_tx_id: 0x11,
                    bounds: Discrete(vec![(0, "Logarithmic"), (1, "Exponential"), (2, "Linear")]),
                },
                Param {
                    name: "MidiRecvChan",
                    sysex_rx_id: 0x05,
                    sysex_tx_id: 0x06,
                    bounds: Range(1, (1, 16)),
                },
                Param {
                    name: "MidiSendChan",
                    sysex_rx_id: 0x07,
                    sysex_tx_id: 0x08,
                    bounds: Range(1, (1, 16)),
                },
                Param {
                    name: "LfoKeyRetrig",
                    sysex_rx_id: 0x0f,
                    sysex_tx_id: 0x10,
                    bounds: Discrete(vec![(0, "Off"), (1, "On")]),
                },
                Param {
                    name: "EnvLegatoMode",
                    sysex_rx_id: 0x0d,
                    sysex_tx_id: 0x0e,
                    bounds: Discrete(vec![(0, "Off"), (1, "On")]),
                },
                Param {
                    name: "BendRange",
                    sysex_rx_id: 0x2c,
                    sysex_tx_id: 0x2d,
                    bounds: Range(1, (1, 12)),
                },
                Param {
                    name: "Gate",
                    sysex_rx_id: 0x36,
                    sysex_tx_id: 0x37,
                    bounds: Discrete(vec![(1, "Short"), (2, "Medium"), (3, "Long")]),
                },
                Param {
                    name: "Sync",
                    sysex_rx_id: 0x3c,
                    sysex_tx_id: 0x3d,
                    bounds: Discrete(vec![(0, "Auto"), (1, "Internal"), (2, "External")]),
                },
                Param {
                    name: "SeqPlay",
                    sysex_rx_id: 0x2e,
                    sysex_tx_id: 0x2f,
                    bounds: Discrete(vec![(0, "Hold"), (1, "NoteOn")]),
                },
                Param {
                    name: "SeqKeyRetrig",
                    sysex_rx_id: 0x34,
                    sysex_tx_id: 0x35,
                    bounds: Discrete(vec![(0, "Reset"), (1, "Legato"), (2, "None")]),
                },
                Param {
                    name: "SeqNextSeq",
                    sysex_rx_id: 0x32,
                    sysex_tx_id: 0x33,
                    bounds: Discrete(vec![(0, "End"), (1, "Reset"), (2, "Continue")]),
                },
                Param {
                    name: "SeqStep",
                    sysex_rx_id: 0x38,
                    sysex_tx_id: 0x39,
                    // TODO possible custom step hack?
                    bounds: Discrete(vec![
                        (0x04, "1/4"),
                        (0x08, "1/8"),
                        (0x10, "1/16"),
                        (0x20, "1/32"),
                    ]),
                },
                Param {
                    name: "SeqStepOn",
                    sysex_rx_id: 0x2a,
                    sysex_tx_id: 0x2b,
                    bounds: Discrete(vec![(0, "Clock"), (1, "Gate")]),
                },
            ],
        }
    }

}

