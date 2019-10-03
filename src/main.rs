extern crate strum;
#[macro_use]
extern crate strum_macros;

mod devices;
mod schema;

use midir::MidiOutput;
use structopt::StructOpt;
use strum::IntoEnumIterator;

use crate::devices::{DeviceError, DeviceType};

#[derive(StructOpt, Debug)]
#[structopt(
    name = "la_bruteforce",
    about = "La BruteForce is used to edit Arturia devices hidden parameters"
)]
enum Cmd {
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
        /// Name of the param as listed
        param_key: String,
        /// New bound value of the param
        value_ids: Vec<String>,
    },
}

use crate::devices::Bounds;
use crate::devices::CLIENT_NAME;
use std::str::FromStr;
use crate::schema::ParamKey;

fn main() -> devices::Result<()> {
    let cmd = Cmd::from_args();

    match cmd {
        Cmd::Ports => {
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            devices::output_ports(&midi_client)
                .iter()
                .for_each(|port| println!("{}", port.name))
        }

        Cmd::Devices => schema::SCHEMAS.keys().iter()
            .for_each(|dev| println!("{}", dev)),

        Cmd::Params { device_name } => {
            let mut dev: schema::Device = schema::SCHEMAS.get(&device_name)?;
            for (name, param) in dev.parameters.clone().entries() {
                if let Some(range) = param.range {
                    for i in range.lo..range.hi {
                        
                    }
                }
                println!("{}", param);
            }
        }
        Cmd::Bounds {
            device_name,
            param_key,
            field_name,
        } => {
            let dev: schema::Device = schema::SCHEMAS.get(&device_name)?;
            let param_key = dev.parse_key(&param_key)?;
            let bounds = param_key.bounds(field_name)?;
            for bound in bounds {
                match bound {
                    schema::Bounds::Values(values) => {
                        for value in values {
                            println!("{}", value.1)
                        }
                    }
                    schema::Bounds(range) => println!("[{}..{}]", range.lo, range.hi),
                    schema::Bounds::NoteSeq(_) => println!("note1 note2 note3 ..."),
                }
            }
        },
        Cmd::Set {
            device_name,
            param_key,
            value_ids,
        } => {
            let dev: schema::Device = schema::SCHEMAS.get(&device_name)?;
            let param_key = dev.parse_key(&param_key)?;
            let mut dev = dev.locate()?.connect()?;
            dev.update(&param_key, &value_ids)?;
        }
        Cmd::Get {
            device_name,
            mut param_names,
        } => {
            let dev: schema::Device = schema::SCHEMAS.get(&device_name)?;
            if param_names.is_empty() {
                param_names = dev.parameters.keys().collect();
            }

            let mut dev = dev.locate()?.connect()?;
            for p in param_names {
                let param_key = dev.parse_key(&p)?;
                dev.query(&param_key)?;
            }


            let dev = DeviceType::from_str(&device_name)?.descriptor();
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            let port = dev
                .ports()
                .get(0)
                .cloned()
                .ok_or(DeviceError::NoOutputPort {
                    port_name: device_name,
                })?;
            let mut sysex = dev.connect(midi_client, &port)?;
            if param_names.is_empty() {
                param_names = dev.globals().iter().map(|p| p.to_string()).collect();
            }
            for pair in sysex.query(param_names.as_slice())? {
                println!("{} {}", pair.0, pair.1.join(" "))
            }
        }
    }

    Ok(())
}
