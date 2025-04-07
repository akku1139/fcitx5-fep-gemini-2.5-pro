// Main entry point for the Fcitx5 FEP application.
// Handles initialization, argument parsing (if any), and starts the main event loop.

mod error;
mod event_loop;
mod fcitx;
mod state;
mod terminal;

use error::FepError;
use event_loop::run_event_loop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Fcitx5 FEP (Conceptual)...");

    // Initialize terminal, Fcitx connection, and state
    let mut terminal = terminal::Terminal::new()?;
    let mut fcitx_client = fcitx::FcitxClient::connect()?;
    let mut app_state = state::AppState::new();

    // Run the main event loop
    run_event_loop(&mut terminal, &mut fcitx_client, &mut app_state)?;

    println!("Exiting Fcitx5 FEP.");
    Ok(())
}
