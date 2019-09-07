#[macro_use]
extern crate lazy_static;

use structopt::StructOpt;
use midir::{MidiOutput, MidiInput};

use crate::devices::ParameterBounds;
use crate::midi::SysexConnection;
use std::error::Error;

mod devices;
mod hotplug;
mod midi;
mod tui;

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
        param_value: String,
    },
}

#[derive(StructOpt, Debug)]
enum List {
    /// All active devices
    Ports,

    /// All known devices
    Devices {},

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

use snafu::{ResultExt, Snafu, OptionExt, IntoError};
use std::io;
use crate::List::Devices;

#[derive(Debug, Snafu)]
enum DeviceError {
    #[snafu(display("Invalid device name {}", device_name))]
    InvalidName {
        device_name: String,
    },
    NoPort {
        device_name: String,
    },
    InvalidParam {
        device_name: String,
        param_name: String,
    },
    NoValueReceived,
    UnknownParameter {
        param_id: u8,
    },
}

type Result<T, E = DeviceError> = std::result::Result<T, E>;

fn main() -> midi::Result<()> {
    let app = LaBruteForce::from_args();
    let cmd: Command = app.subcmd.unwrap_or(Command::TUI);

    match cmd {
        Command::TUI {} => {
            let mut tui = tui::build_tui();
            tui.run();
        }
        Command::Watch { device } => {
            hotplug::watch();
        }
        Command::List { subcmd } => {
            let subcmd = subcmd.unwrap_or(List::Ports);
            match subcmd {
                List::Ports {} => {
                    let midi_out = MidiOutput::new("LaBruteForce")?;
                    let ports = midi::output_ports(&midi_out)
                        .iter()
                        .for_each(|(name, _idx)| println!("{}", name));
                }
                List::Devices {} => devices::known_devices()
                    .iter()
                    .for_each(|dev| println!("{}", dev.name)),
                List::Params { device_name } => {
                    devices::known_devices_by_name().get(&device_name)
                        .map(|dev| for param in &dev.params {
                            println!("{}", param.name);
                        })
                        .unwrap_or_else(|| println!("Unknown device '{}'. Use `la_bruteforce list device` for known device names", device_name));
                }
                List::Bounds {
                    device_name,
                    param_name,
                } => {
                    devices::known_devices_by_name().get(&device_name)
                        .map(|dev| dev.params.iter()
                            .find(|param| param_name.as_str().eq(param.name))
                            .map(|param| match &param.bounds {
                                ParameterBounds::Discrete(values) => {
                                    for bound in values {
                                        println!("{}", bound.1)
                                    }},
                                ParameterBounds::Range(lo, hi) => println!("[{}..{}]", lo, hi)
                            })
                            .unwrap_or_else(|| println!("Unknown param '{}'. Use `la_bruteforce list params {}` for known param names", param_name, device_name))
                        )
                        .unwrap_or_else(|| println!("Unknown device '{}'. Use `la_bruteforce list devices` for known device names", device_name));
                }
            };
        }
        Command::Set {
            device_name,
            param_name,
            param_value,
        } => {
            hotplug::watch();
        }
        Command::Get {
            device_name,
            mut param_names,
        } => {
            let device = devices::known_devices_by_name()
                .get(&device_name)
                .cloned()
                .ok_or(DeviceError::InvalidName{device_name: device_name.clone()})?;

            let midi_out = MidiOutput::new(midi::CLIENT_NAME)?;
            let (port_name, port_idx) = midi::output_ports(&midi_out).iter()
                .find(|(pname, idx)| pname.starts_with(&device.port_name))
                .map(|(pname, idx)| (pname.clone(), *idx))
                .ok_or(DeviceError::NoPort { device_name: device_name.clone() })?;
            let mut sysex = midi_out.connect(port_idx, &port_name)
                .map(|conn| SysexConnection::new(conn, device.clone()))?;
            let rx = sysex.init_receiver(&port_name, &device)?;

            if param_names.is_empty() {
                for param in &device.params {
                    sysex.query_value(param.sysex_tx_id)?;
                }
            } else {
                for param_name in param_names {
                    let param = device.params.iter().find(|p| p.name.eq(&param_name))
                        .cloned()
                        .ok_or(DeviceError::InvalidParam {
                            device_name: device_name.clone(),
                            param_name: param_name.clone(),
                        })?;

                    sysex.query_value(param.sysex_tx_id)?;
                }
            }

            let results = rx.close(1000);
            if results.is_empty(){
                return Err(Box::new(DeviceError::NoValueReceived))
            }

            for (param_id, value_id) in results.iter() {
                let param = device.params.iter()
                    .find(|pid| pid.sysex_rx_id == *param_id)
                    .ok_or(DeviceError::UnknownParameter {param_id: *param_id})?;

                match &param.bounds {
                    ParameterBounds::Discrete(values) => {
                        let bound = values.iter().find(|v| v.0 == *value_id).map_or_else(
                            || format!("## UNBOUND_PARAM_VALUE [{}]", value_id),
                            |v| v.1.to_owned());
                        println!("{}: {}", &param.name, bound)
                    },
                    ParameterBounds::Range(lo, hi) => println!("{}: {}", &param.name, *value_id)
                };
            }

        }
        _ => (),
    }

    Ok(())
}
