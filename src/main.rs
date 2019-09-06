#[macro_use]
extern crate lazy_static;

use structopt::StructOpt;

use crate::devices::ParameterBounds;
use midir::MidiOutput;

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
        param_name: String,
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

fn main() -> midi::Result<()> {
    let app = LaBruteForce::from_args();
    let cmd: Command = app.subcmd.unwrap_or(Command::TUI);
    //    println!("{:#?}", cmd);

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
            param_name,
        } => {
            hotplug::watch();
        }
        _ => (),
    }

    Ok(())
}
