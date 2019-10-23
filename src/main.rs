extern crate strum;
#[macro_use]
extern crate strum_macros;

#[macro_use]
extern crate lazy_static;

mod devices;
mod schema;
mod parse;

use midir::MidiOutput;
use structopt::StructOpt;
use strum::IntoEnumIterator;

use crate::devices::DeviceError;
use crate::devices::CLIENT_NAME;

use crate::schema::{Bounds, Device};
use crate::parse::{Token, ParseError};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "la_bruteforce",
    about = "La BruteForce is used to edit Arturia devicf hidden parameters"
)]
enum Cmd {
    /// All active devicf
    Ports,

    /// All known devicf
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
        param_key: String,
        /// Name of field to get bounds of
        field_name: Option<String>,
    },

    #[structopt(name = "get")]
    /// Get a device's parameter value
    Get {
        /// Name of the device as listed
        device_name: String,
        /// Name of the param as listed
        param_keys: Vec<String>,
    },

    #[structopt(name = "set")]
    /// Set a device's parameter value
    Set {
        /// Name of the device as listed
        device_name: String,
        /// New bound value of the param
        key_and_value: Vec<String>,
    },
}

fn main() -> devices::Result<()> {
    let cmd = Cmd::from_args();

    match cmd {
        Cmd::Ports => {
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            devices::output_ports(&midi_client)
                .iter()
                .for_each(|port| println!("{}", port.name))
        }

        Cmd::Devices => schema::DEVICES.keys().for_each(|dev| println!("{}", dev)),

        Cmd::Params { device_name } => {
            let (vendor, dev) = schema::DEVICES
                .get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            if let Some(controls) = dev.controls {
                for control in controls {
                    print!("{}", control.name);
                    println!()
                }
            }
            if let Some(controls) = dev.indexed_controls {
                for control in controls {
                    print!("{}", control.name);
                    print!("/[{}..{}]", control.index.lo, control.index.hi);
                    println!()
                }
            }
            // TODO modes
        }
        Cmd::Bounds {
            device_name,
            param_key,
            field_name,
        } => {
            let dev = schema::DEVICES
                .get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            let param_key = dev.parse_key(&param_key)?;
            let bounds: Vec<Bounds> = if let Some(field) = field_name {
                param_key.bounds(Some(&field))
            } else {
                param_key.bounds(None)
            }?;
            for bound in bounds {
                match bound {
                    schema::Bounds::Values(values) => {
                        for value in values {
                            println!("{}", value.1)
                        }
                    }
                    schema::Bounds::Range(range) => println!("[{}..{}]", range.lo, range.hi),
                    schema::Bounds::MidiNotes(_) => println!("note1,note2,note3,..."),
                }
            }
        }
        Cmd::Set {
            device_name,
            mut key_and_value,
        } => {
            let tokens = parse::parse_query(&device_name, &mut key_and_value)?;
            let device = *tokens.iter()
                .find_map(|token| if let Token::Device(d) = token {Some(d)} else {None})
                .ok_or(ParseError::MissingDevice);
            let mut dev = devices::locate(device)?.connect()?;
            dev.update(&tokens)?;
        }
        Cmd::Get {
            device_name,
            mut param_keys,
        } => {
            let tokens = parse::parse_query(&device_name, param_keys.as_mut_slice())?;

            let dev = schema::DEVICES
                .get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            if param_keys.is_empty() {
                param_keys = dev
                    .parameters
                    .iter()
                    .flat_map(|(name, param)| {
                        if let Some(range) = param.range {
                            (range.lo..range.hi + 1)
                                .map(|index| format! {"{}/{}", name, index})
                                .collect()
                        } else {
                            vec![name.to_string()]
                        }
                    })
                    .collect();
            }

            let mut loc = dev.locate()?.connect()?;
            for p in param_keys {
                let param_key = dev.parse_key(&p)?;
                let values = loc.query(&param_key)?;
                println!("{} {}", param_key, values.join(" "));
            }
        }
    }

    Ok(())
}
