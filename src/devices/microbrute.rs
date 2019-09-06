use crate::devices::ParameterBounds::*;
use crate::devices::{Device, Param};

pub fn microbrute() -> Device {
    Device {
        name: "MicroBrute",
        usb_vendor_id: 0x1c75,
        usb_product_id: 0x0206,
        sysex_out_id: 0x05,
        sysex_cmd_id: 0x06,
        params: vec![
            Param {
                name: "NotePriority",
                sysex_out_id: 0x0b,
                sysex_cmd_id: 0x0c,
                bounds: Discrete(vec![(0, "LastNote"), (1, "LowNote"), (2, "HighNote")]),
            },
            Param {
                name: "VelocityResponse",
                sysex_out_id: 0x10,
                sysex_cmd_id: 0x11,
                bounds: Discrete(vec![(0, "Logarithmic"), (1, "Exponential"), (2, "Linear")]),
            },
            Param {
                name: "Play",
                sysex_out_id: 0x2e,
                sysex_cmd_id: 0x2f,
                bounds: Discrete(vec![(0, "Hold"), (1, "Note On")]),
            },
            Param {
                name: "SeqRetrig",
                sysex_out_id: 0x34,
                sysex_cmd_id: 0x35,
                bounds: Discrete(vec![(0, "Reset"), (1, "Legato"), (2, "None")]),
            },
            Param {
                name: "NextSeq",
                sysex_out_id: 0x32,
                sysex_cmd_id: 0x33,
                bounds: Discrete(vec![(0, "End"), (1, "Reset"), (2, "Continue")]),
            },
            Param {
                name: "StepOn",
                sysex_out_id: 0x2a,
                sysex_cmd_id: 0x2b,
                bounds: Discrete(vec![(0, "Clock"), (1, "Gate")]),
            },
            Param {
                name: "Step",
                sysex_out_id: 0x38,
                sysex_cmd_id: 0x39,
                // TODO possible custom step hack?
                bounds: Discrete(vec![
                    (0x04, "1/4"),
                    (0x08, "1/8"),
                    (0x10, "1/16"),
                    (0x20, "1/32"),
                ]),
            },
            Param {
                name: "LfoKeyRetrig",
                sysex_out_id: 0x0f,
                sysex_cmd_id: 0x10,
                bounds: Discrete(vec![(0, "Off"), (1, "On")]),
            },
            Param {
                name: "EnvLegatoMode",
                sysex_out_id: 0x0d,
                sysex_cmd_id: 0x0e,
                bounds: Discrete(vec![(0, "Off"), (1, "On")]),
            },
            Param {
                name: "Gate",
                sysex_out_id: 0x36,
                sysex_cmd_id: 0x37,
                bounds: Discrete(vec![(1, "Short"), (2, "Medium"), (3, "Long")]),
            },
            Param {
                name: "Sync",
                sysex_out_id: 0x3c,
                sysex_cmd_id: 0x3d,
                bounds: Discrete(vec![(0, "Auto"), (1, "Internal"), (2, "External")]),
            },
            Param {
                name: "BendRange",
                sysex_out_id: 0x2c,
                sysex_cmd_id: 0x2d,
                bounds: Range(1, 12),
            },
            Param {
                name: "MidiRecvChan",
                sysex_out_id: 0x05,
                sysex_cmd_id: 0x06,
                bounds: Range(1, 16),
            },
            Param {
                name: "MidiSendChan",
                sysex_out_id: 0x07,
                sysex_cmd_id: 0x08,
                bounds: Range(1, 16),
            },
        ],
    }
}
