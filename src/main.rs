extern crate strum;
#[macro_use]
extern crate strum_macros;

#[macro_use]
extern crate lazy_static;

mod devices;
mod schema;

use midir::MidiOutput;
use structopt::StructOpt;
use strum::IntoEnumIterator;

use crate::devices::{DeviceError};

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

use crate::devices::CLIENT_NAME;
use crate::schema::Bounds;

fn main() -> devices::Result<()> {
    let cmd = Cmd::from_args();

    match cmd {
        Cmd::Ports => {
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            devices::output_ports(&midi_client)
                .iter()
                .for_each(|port| println!("{}", port.name))
        }

        Cmd::Devices => schema::SCHEMAS.keys()
            .for_each(|dev| println!("{}", dev)),

        Cmd::Params { device_name } => {
            let dev = schema::SCHEMAS.get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            for (name, param) in dev.parameters.iter() {
                print!("{}", name);
                if let Some(range) = param.range {
                    print!("/{}..{}", range.lo, range.hi);
                }
                if let Some(modes) = &param.modes {
                    let z: Vec<String> = modes.keys().map(|s| s.to_string()).collect();
                    print!(":[{}]", z.join("|"));
                }
            }
        }
        Cmd::Bounds {
            device_name,
            param_key,
            field_name,
        } => {
            let dev = schema::SCHEMAS.get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            let param_key = dev.parse_key(&param_key)?;
            let bounds: Vec<Bounds> = if let Some(field) = field_name {param_key.bounds(Some(&field))} else {param_key.bounds(None)}?;
            for bound in bounds {
                match bound {
                    schema::Bounds::Values(values) => {
                        for value in values {
                            println!("{}", value.1)
                        }
                    }
                    schema::Bounds::Range(range) => println!("[{}..{}]", range.lo, range.hi),
                    schema::Bounds::NoteSeq(_) => println!("note1,note2,note3,..."),
                }
            }
        },
        Cmd::Set {
            device_name,
            param_key,
            value_ids,
        } => {
            let dev = schema::SCHEMAS.get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            let param_key = dev.parse_key(&param_key)?;
            let mut dev = dev.locate()?.connect()?;
            dev.update(&param_key, &value_ids)?;
        }
        Cmd::Get {
            device_name,
            mut param_keys,
        } => {
            let dev = schema::SCHEMAS.get(&device_name)
                .ok_or(DeviceError::UnknownDevice { device_name })?;
            if param_keys.is_empty() {
                param_keys = dev.parameters.iter()
                    .flat_map(|(name, param)|
                        if let Some(range) = param.range {
                            (range.lo..range.hi + 1)
                                .map(|index| format!{"{}/{}", name, index} )
                                .collect()
                        } else {
                            vec![name.to_string()]
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
