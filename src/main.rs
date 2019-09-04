use std::path::PathBuf;
use structopt::StructOpt;

use cursive::Cursive;
use cursive::views::{Dialog, TextView};

mod hotplug;

#[derive(StructOpt, Debug)]
#[structopt(name = "la_bruteforce")]
enum CLI {
    #[structopt(name = "watch")]
    /// Monitor known devices being connected and disconnected
    Watch {
        #[structopt(default_value = "*")]
        device: String,
    },
    #[structopt(name = "tui")]
    /// Start Text UI
    TUI,
    #[structopt(name = "show")]
    /// Show information about known devices
    Show {
    },
    #[structopt(name = "list")]
    /// List connected devices
    List {
    },
    #[structopt(name = "get")]
    /// Get a device's parameter value
    Get {
    },
    #[structopt(name = "set")]
    /// Set a device's parameter value
    Set {
    },
}

fn main() {
    let opt = CLI::from_args();
    println!("{:#?}", opt);

    match opt {
        Watch => {hotplug::watch();},
        _ =>  ()
    }

    // Creates the cursive root - required for every application.
    let mut siv = Cursive::default();
    // Creates a dialog with a single "Quit" button
    siv.add_layer(Dialog::around(TextView::new("Hello Dialog!"))
        .title("Cursive")
        .button("Quit", |s| s.quit()));

    // Starts the event loop.
    siv.run();
}