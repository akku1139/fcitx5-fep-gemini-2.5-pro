// src/main.rs
// Main entry point for the async Fcitx5 FEP application.
// Handles initialization, argument parsing (if any), and starts the main event loop.

mod error;
mod event_loop;
mod fcitx;
mod state;
mod terminal;

use error::FepError;
use event_loop::run_event_loop;
use tokio::select; // Import tokio::select

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Fcitx5 FEP (Async)...");

    // Initialize terminal (synchronous setup)
    let mut terminal = match terminal::Terminal::new() {
        Ok(term) => term,
        Err(e) => {
            eprintln!("Failed to initialize terminal: {}", e);
            // Attempt to disable raw mode if it was partially enabled, though unlikely here
            let _ = crossterm::terminal::disable_raw_mode();
            return Err(e.into());
        }
    };

    // Connect to Fcitx (asynchronous)
    // Use a block to ensure client is dropped before terminal cleanup if connect fails
    let mut fcitx_client = match fcitx::FcitxClient::connect().await {
         Ok(client) => client,
         Err(e) => {
             eprintln!("Failed to connect to Fcitx: {}", e);
             // Terminal cleanup will happen automatically via Drop
             return Err(e.into());
         }
    };

    let mut app_state = state::AppState::new();

    // Run the main event loop, handling Ctrl+C for graceful shutdown
    println!("FEP started. Press Ctrl+C to exit.");
    select! {
        result = run_event_loop(&mut terminal, &mut fcitx_client, &mut app_state) => {
            if let Err(e) = result {
                eprintln!("\nEvent loop terminated with error: {}", e);
                // Error occurred, return it (cleanup via Drop)
                // Ensure newline after potential raw mode output mess
                println!();
                return Err(e.into());
            } else {
                 // Event loop exited normally (e.g., stream ended)
                 println!("\nEvent loop finished normally.");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\nCtrl+C received, shutting down gracefully...");
            // Signal received, cleanup will happen via Drop
        }
    }

    // Explicit disconnect might be needed if Drop doesn't handle async cleanup well
    // fcitx_client.disconnect().await; // Call if needed

    println!("Exiting Fcitx5 FEP application.");
    // Terminal and FcitxClient cleanup happens via their Drop implementations here
    Ok(())
}
