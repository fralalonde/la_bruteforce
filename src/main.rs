#[macro_use]
extern crate lazy_static;

use structopt::StructOpt;

use cursive::views::{Dialog, TextView};
use cursive::Cursive;
use midir::MidiOutput;

mod devices;
mod hotplug;
mod midi;
mod tui;

#[derive(StructOpt, Debug)]
#[structopt(name = "la_bruteforce")]
enum CLI {
    #[structopt(name = "watch")]
    /// Monitor known devices being connected and disconnected
    Watch { device: Option<String> },
    #[structopt(name = "tui")]
    /// Start Text UI
    TUI,
    #[structopt(name = "show")]
    /// Show information about known devices
    Show {},
    #[structopt(name = "list")]
    /// List connected devices
    List {},
    #[structopt(name = "get")]
    /// Get a device's parameter value
    Get {},
    #[structopt(name = "set")]
    /// Set a device's parameter value
    Set {},
}

fn main() -> midi::Result<()> {
    let opt: CLI = CLI::from_args();
    println!("{:#?}", opt);

    match opt {
        CLI::TUI {} => {
            let mut tui = tui::build_tui();
            tui.run();
        }
        CLI::Watch { device } => {
            hotplug::watch();
        }
        CLI::List {} => {
            let midi_out = MidiOutput::new("LaBruteforce")?;
            let ports = midi::output_ports(&midi_out);
            devices::port_devices(&ports)
                .iter()
                .for_each(|(idx, dev)| println!("idx {} :: name {}", idx, dev.name))
        }
        CLI::Show {} => {
            let midi_out = MidiOutput::new("LaBruteforce")?;
            midi::output_ports(&midi_out)
                .iter()
                .for_each(|(name, idx)| println!("idx {} :: name {}", idx, name))
        }
        _ => (),
    }

    Ok(())
}
