extern crate strum;
#[macro_use]
extern crate strum_macros;

mod devices;
mod hotplug;
mod midi;
mod tui;

use midir::{ MidiOutput};
use structopt::StructOpt;
use strum::{IntoEnumIterator};

use crate::devices::{DeviceError, DeviceType};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "la_bruteforce",
    about = "La BruteForce is used to edit Arturia devices hidden parameters"
)]
struct LaBruteForce {
    // global switches go here
    // (none for now)
    #[structopt(subcommand)] // Note that we mark a field as a subcommand
    subcmd: Option<Command>,
}

#[derive(StructOpt, Debug)]
enum Command {
    #[structopt(name = "tui")]
    /// Start Text UI (default)
    TUI,

    #[structopt(name = "watch")]
    /// Monitor known devices being connected and disconnected
    Watch { device: Option<String> },

    #[structopt(name = "list")]
    /// List connected devices
    List {
        #[structopt(subcommand)] // Note that we mark a field as a subcommand
        subcmd: Option<List>,
    },

//    /// Inquire Sysex General Information
//    Detect { device_name: String },

    /// Scan all possible parameters & possibly break it
//    Scan { device_name: String },
//    /// Reset all known parameters to first bound value
//    Reset { device_name: String },

    #[structopt(name = "get")]
    /// Get a device's parameter value
    Get {
        /// Name of the device as listed
        device_name: String,
        /// Name of the param as listed
        param_names: Vec<String>,
    },

    #[structopt(name = "set")]
    /// Set a device's parameter value
    Set {
        /// Name of the device as listed
        device_name: String,
        /// Name of the param as listed
        param_name: String,
        /// New bound value of the param
        value_name: String,
    },
}

#[derive(StructOpt, Debug)]
enum List {
    /// All active devices
    Ports,

    /// All known devices
    Devices,

    /// A single device's possible parameters
    Params {
        /// Name of the device as listed
        device_name: String,
    },

    Bounds {
        /// Name of the device as listed
        device_name: String,
        /// Name of the param as listed
        param_name: String,
    },
}

use std::str::FromStr;
use crate::devices::Bounds;
use crate::midi::CLIENT_NAME;

fn main() -> midi::Result<()> {
    let app = LaBruteForce::from_args();
    let cmd: Command = app.subcmd.unwrap_or(Command::TUI);

    match cmd {
        Command::TUI {} => {
            let mut tui = tui::build_tui();
            tui.run();
        },
        Command::Watch { device: _ } => {
            hotplug::watch()?;
        },
        Command::List { subcmd } => {
            let subcmd = subcmd.unwrap_or(List::Ports);
            match subcmd {
                List::Ports => {
                    let midi_client = MidiOutput::new(CLIENT_NAME)?;
                    midi::output_ports(&midi_client).iter()
                        .for_each(|port| println!("{}", port.name))
                },
                List::Devices => DeviceType::iter()
                    .for_each(|dev| println!("{}", dev)),
                List::Params { device_name } => {
                    let dev = DeviceType::from_str(&device_name)?;
                    for param in dev.descriptor().parameters() {
                        println!("{}", param);
                    }
                }
                List::Bounds { device_name, param_name } => {
                    let dev = DeviceType::from_str(&device_name)?;
                    match dev.descriptor().bounds(&param_name)? {
                        Bounds::Discrete(values) =>
                            for bound in values {
                                println!("{}", bound.1)
                            },
                        Bounds::Range(_offset, (lo, hi)) => println!("[{}..{}]", lo, hi)
                    }
                }
            };
        },
        Command::Set {
            device_name,
            param_name,
            value_name,
        } => {
            let dev = DeviceType::from_str(&device_name)?.descriptor();
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            let mut sysex = dev.connect(midi_client, dev.ports().get(0).expect("FUCK RUST ERRORS"))?;
            sysex.update(&param_name, &value_name)?;
        },
        Command::Get {
            device_name,
            param_names,
        } => {
            let dev = DeviceType::from_str(&device_name)?.descriptor();
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            let port = dev.ports().get(0).cloned()
                .ok_or(DeviceError::NoOutputPort {port_name: device_name})?;
            let mut sysex = dev.connect(midi_client, &port)?;
            for pair in sysex.query(param_names.as_slice())? {
                println!("{} {}", pair.0, pair.1)
            }
        },
    }

    Ok(())
}

//        Command::Scan { device_name } => {
//            query(&device_name, |sysex, device| {
//                for sysex_tx_id in 06..0xFF {
//                    sysex.query_value(sysex_tx_id)?;
//                }
//                Ok(())
//            });
//        }
//        Command::Reset { device_name } => {
//            query(&device_name, |sysex, device| {
//                for sysex_tx_id in 06..0xFF {
//                    sysex.query_value(sysex_tx_id)?;
//                }
//                Ok(())
//            });
//        }
