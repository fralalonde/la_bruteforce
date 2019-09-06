use cursive::views::{Dialog, TextView};
use cursive::Cursive;

pub fn build_tui() -> Cursive {
    // Creates the cursive root - required for every application.
    let mut tui = Cursive::default();
    // Creates a dialog with a single "Quit" button
    tui.add_layer(
        Dialog::around(TextView::new("La Bruteforce iz in de house!"))
            .title("Brute")
            .button("Force", |s| s.quit()),
    );
    tui
}
