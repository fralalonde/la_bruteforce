#[macro_use]
extern crate lazy_static;

use structopt::StructOpt;

use midir::MidiOutput;
use crate::devices::ParameterBounds;

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
    Get {},
    #[structopt(name = "set")]
    /// Set a device's parameter value
    Set {},
}

#[derive(StructOpt, Debug)]
enum List {
    /// All active devices
    Port,

    /// All known devices
    Device {},

    /// A single device's possible parameters
    Param {
        /// Name of the known device as listed
        device_name: String,
    },
    Bound {
        /// Name of the known device as listed
        device_name: String,
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
            let subcmd = subcmd.unwrap_or(List::Port);
            match subcmd {
                List::Port {} => {
                    let midi_out = MidiOutput::new("LaBruteForce")?;
                    let ports = midi::output_ports(&midi_out)
                        .iter()
                        .for_each(|(name, _idx)| println!("{}", name));
                }
                List::Device {} => devices::known_devices()
                    .iter()
                    .for_each(|dev| println!("{}", dev.name)),
                List::Param { device_name } => {
                    devices::known_devices_by_name().get(&device_name)
                        .map(|dev| for param in &dev.params {
                            println!("{}", param.name);
                        })
                        .unwrap_or_else(|| println!("Unknown device '{}'. Use `la_bruteforce list device` for known device names", device_name));
                }
                List::Bound { device_name, param_name } => {
                    devices::known_devices_by_name().get(&device_name)
                        .map(|dev| dev.params.iter().find(|param| param.name.equals(param_name))
                            .map(|param| match &param.bounds {
                            &ParameterBounds::Discrete(values) =>
                                for v in &values {
                                    println!("{}", param.1);
                                }
                            &ParameterBounds::Range(low, hi) => println!("[{}..{}]", lo, hi)
                        })
                            .unwrap_or_else(|| println!("Unknown param '{}'. Use `la_bruteforce list param {}` for known param names", param_name, device_name));
                        )
                        .unwrap_or_else(|| println!("Unknown device '{}'. Use `la_bruteforce list device` for known device names", device_name));
                }
            };
        }
        _ => (),
    }

    Ok(())
}
