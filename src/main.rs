#![feature(arbitrary_enum_discriminant)]
#![feature(generators, generator_trait)]

#[macro_use]
extern crate lazy_static;

use midir::{MidiInput, MidiOutput};
use structopt::StructOpt;

use crate::devices::{DeviceDescriptor, ParameterBounds};
use crate::midi::SysexConnection;
use std::error::Error;

use std::pin::Pin;
use std::ops::Generator;

mod devices;
mod hotplug;
mod midi;
mod sysex;
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

    /// Inquire Sysex General Information
    Detect { device_name: String },

    /// Scan all possible parameters & possibly break it
    Scan { device_name: String },
    /// Reset all known parameters to first bound value
    Reset { device_name: String },
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

use crate::List::Devices;
use snafu::{IntoError, OptionExt, ResultExt, Snafu};
use std::io;
use std::str::FromStr;
use std::ops::GeneratorState;

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
    InvalidValue {
        value_name: String,
    },
}

type Result<T, E = DeviceError> = std::result::Result<T, E>;

fn main() -> midi::Result<()> {
    let app = LaBruteForce::from_args();
    let cmd: Command = app.subcmd.unwrap_or(Command::TUI);

//    let mut generator = || {
//        yield 1;
//        return "foo"
//    };
//
//    match Pin::new(&mut generator).resume() {
//        GeneratorState::Yielded(1) => {}
//        _ => panic!("unexpected value from resume"),
//    }
//    match Pin::new(&mut generator).resume() {
//        GeneratorState::Complete("foo") => {}
//        _ => panic!("unexpected value from resume"),
//    }

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
                                ParameterBounds::Range(_offset, (lo, hi)) => println!("[{}..{}]", lo, hi)
                            })
                            .unwrap_or_else(|| println!("Unknown param '{}'. Use `la_bruteforce list params {}` for known param names", param_name, device_name))
                        )
                        .unwrap_or_else(|| println!("Unknown device '{}'. Use `la_bruteforce list devices` for known device names", device_name));
                }
            };
        }
        Command::Detect { device_name } => {
            let device = devices::known_devices_by_name()
                .get(&device_name)
                .cloned()
                .ok_or(DeviceError::InvalidName {
                    device_name: device_name.to_owned(),
                })?;

            let midi_out = MidiOutput::new(midi::CLIENT_NAME)?;
            let (port_name, port_idx) = midi::output_ports(&midi_out)
                .iter()
                .find(|(pname, idx)| pname.starts_with(&device.port_name_prefix))
                .map(|(pname, idx)| (pname.clone(), *idx))
                .ok_or(DeviceError::NoPort {
                    device_name: device_name.to_owned(),
                })?;

            let mut sysex = midi_out
                .connect(port_idx, &port_name)
                .map(|conn| SysexConnection::new(conn, device.clone()))?;

            sysex.query_general_information();
        }
        Command::Set {
            device_name,
            param_name,
            value_name,
        } => {
            let device = devices::known_devices_by_name()
                .get(&device_name)
                .cloned()
                .ok_or(DeviceError::InvalidName {
                    device_name: device_name.to_owned(),
                })?;

            let midi_out = MidiOutput::new(midi::CLIENT_NAME)?;
            let (port_name, port_idx) = midi::output_ports(&midi_out)
                .iter()
                .find(|(pname, idx)| pname.starts_with(&device.port_name_prefix))
                .map(|(pname, idx)| (pname.clone(), *idx))
                .ok_or(DeviceError::NoPort {
                    device_name: device_name.to_owned(),
                })?;

            let mut sysex = midi_out
                .connect(port_idx, &port_name)
                .map(|conn| SysexConnection::new(conn, device.clone()))?;

            let param = device
                .params
                .iter()
                .find(|p| p.name.eq(&param_name))
                .cloned()
                .ok_or(DeviceError::InvalidParam {
                    device_name: device_name.clone(),
                    param_name: param_name.clone(),
                })?;

            let vbound = match &param.bounds {
                ParameterBounds::Discrete(values) => values
                    .iter()
                    .find(|v| v.1.eq(&value_name))
                    .map(|v| v.0)
                    .ok_or(DeviceError::InvalidValue { value_name })?,
                ParameterBounds::Range(offset, (_lo, _hi)) => {
                    u8::from_str(&value_name)? - *offset
                }
            };

            sysex.send_value(param.sysex_tx_id, vbound)?;
        }
        Command::Get {
            device_name,
            mut param_names,
        } => {
            query(&device_name, |sysex, device| {
                if param_names.is_empty() {
                    for param in &device.params {
                        sysex.query_value(param.sysex_tx_id)?;
                    }
                } else {
                    for param_name in param_names {
                        let param = device
                            .params
                            .iter()
                            .find(|p| p.name.eq(&param_name))
                            .cloned()
                            .ok_or(DeviceError::InvalidParam {
                                device_name: device_name.clone(),
                                param_name: param_name.clone(),
                            })?;

                        sysex.query_value(param.sysex_tx_id)?;
                    }
                }
                Ok(())
            });
        }
        Command::Scan { device_name } => {
            query(&device_name, |sysex, device| {
                for sysex_tx_id in 06..0xFF {
                    sysex.query_value(sysex_tx_id)?;
                }
                Ok(())
            });
        }
        Command::Reset { device_name } => {
            query(&device_name, |sysex, device| {
                for sysex_tx_id in 06..0xFF {
                    sysex.query_value(sysex_tx_id)?;
                }
                Ok(())
            });
        }

        _ => (),
    }

    Ok(())
}

fn query<F: FnOnce(&mut SysexConnection, &DeviceDescriptor) -> midi::Result<()>>(
    device_name: &str,
    rece: F,
) -> midi::Result<()> {
    let device = devices::known_devices_by_name()
        .get(device_name)
        .cloned()
        .ok_or(DeviceError::InvalidName {
            device_name: device_name.to_owned(),
        })?;

    let midi_out = MidiOutput::new(midi::CLIENT_NAME)?;
    let (port_name, port_idx) = midi::output_ports(&midi_out)
        .iter()
        .find(|(pname, idx)| pname.starts_with(&device.port_name_prefix))
        .map(|(pname, idx)| (pname.clone(), *idx))
        .ok_or(DeviceError::NoPort {
            device_name: device_name.to_owned(),
        })?;
    let mut sysex = midi_out
        .connect(port_idx, &port_name)
        .map(|conn| SysexConnection::new(conn, device.clone()))?;
    let rx = sysex.init_receiver(&port_name, &device)?;

    rece(&mut sysex, &device);

    let results = rx.close(1000);
    if results.is_empty() {
        return Err(Box::new(DeviceError::NoValueReceived));
    }

    for (param_id, value_id) in results.iter() {
        match device
            .params
            .iter()
            .find(|pid| pid.sysex_rx_id == *param_id)
        {
            Some(param) => match &param.bounds {
                ParameterBounds::Discrete(values) => {
                    let bound = values.iter().find(|v| v.0 == *value_id).map_or_else(
                        || format!("## UNBOUND_PARAM_VALUE [{}]", value_id),
                        |v| v.1.to_owned(),
                    );
                    println!("{}: {}", &param.name, bound)
                }
                ParameterBounds::Range(offset, (lo, hi)) => {
                    println!("{}: {}", &param.name, *value_id + *offset)
                }
            },
            None => println!("##Unknown[{}]: {}", param_id, value_id),
        }
    }
    Ok(())
}
