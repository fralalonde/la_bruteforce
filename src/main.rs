extern crate strum;
#[macro_use]
extern crate strum_macros;

mod device;
mod error;
mod midi;

use midir::MidiOutput;
use structopt::StructOpt;
use strum::IntoEnumIterator;

use crate::device::{DeviceType};
use crate::error::{DeviceError};

pub type Result<T> = ::std::result::Result<T, Box<dyn ::std::error::Error>>;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "la_bruteforce",
    about = "La BruteForce is used to edit Arturia device hidden parameters"
)]
enum Cmd {
    /// All active device
    Ports,

    /// All known device
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

use crate::device::Bounds;
use crate::midi::CLIENT_NAME;
use std::str::FromStr;

fn main() -> Result<()> {
    let cmd = Cmd::from_args();

    match cmd {
        Cmd::Ports => {
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            midi::output_ports(&midi_client)
                .iter()
                .for_each(|port| println!("{}", port.name))
        }
        Cmd::Devices => DeviceType::iter().for_each(|dev| println!("{}", dev)),
        Cmd::Params { device_name } => {
            let dev = DeviceType::from_str(&device_name)?;
            for param in dev.descriptor().parameters() {
                println!("{}", param);
            }
        }
        Cmd::Bounds {
            device_name,
            param_name,
        } => {
            let dev = DeviceType::from_str(&device_name)?;
            match dev.descriptor().bounds(&param_name)? {
                Bounds::Discrete(values) => {
                    for bound in values {
                        println!("{}", bound.1)
                    }
                }
                Bounds::Range(_offset, (lo, hi)) => println!("[{}..{}]", lo, hi),
            }
        }
        Cmd::Set {
            device_name,
            param_name,
            value_name,
        } => {
            let dev = DeviceType::from_str(&device_name)?.descriptor();
            let midi_client = MidiOutput::new(CLIENT_NAME)?;
            let mut sysex =
                dev.connect(midi_client, dev.ports().get(0).expect("FUCK RUST ERRORS"))?;
            sysex.update(&param_name, &value_name)?;
        }
        Cmd::Get {
            device_name,
            mut param_names,
        } => {
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
                param_names = dev.parameters()
            }
            sysex.query(param_names.as_slice())?
        }
    }

    Ok(())
}
