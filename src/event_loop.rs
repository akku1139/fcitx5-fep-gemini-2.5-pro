// The main event loop that handles input from the terminal and Fcitx5.

use crate::error::FepError;
use crate::fcitx::FcitxClient;
use crate::state::AppState;
use crate::terminal::Terminal;

pub fn run_event_loop(
    terminal: &mut Terminal,
    fcitx_client: &mut FcitxClient,
    app_state: &mut AppState,
) -> Result<(), FepError> {
    println!("Entering event loop. Press Ctrl+C to exit.");
    println!("NOTE: This is a basic loop. Input handling and Fcitx interaction are placeholders.");

    // Initial render
    terminal.render(app_state)?;

    loop {
        // In a real FEP, this needs non-blocking I/O or threading/async
        // to handle both terminal input and Fcitx signals concurrently.

        // 1. Check for Fcitx Updates (Non-blocking or with timeout)
        match fcitx_client.receive_update() {
            Ok(Some(update)) => {
                app_state.apply_update(update);
                terminal.render(app_state)?; // Re-render after Fcitx update
            }
            Ok(None) => {
                // No update from Fcitx, continue to check terminal input
            }
            Err(e) => {
                eprintln!("Error receiving Fcitx update: {}", e);
                // Decide how to handle error (e.g., break loop, try reconnecting)
                break; // Exit loop on error for simplicity
            }
        }

        // 2. Check for Terminal Input (Simplified blocking read)
        //    A real implementation needs non-blocking read or select/poll.
        //    This simple version blocks here until user presses Enter.
        println!("Waiting for terminal input (press Enter after typing):");
        match terminal.read_input() {
            Ok(Some(input)) => {
                if input == "exit" { // Simple exit condition
                    println!("Exit command received.");
                    break;
                }

                // Forward input to Fcitx (Placeholder)
                if let Err(e) = fcitx_client.forward_key_event(&input) {
                    eprintln!("Error forwarding key event: {}", e);
                    // Handle error appropriately
                    break; // Exit loop on error for simplicity
                }

                // --- Conceptual Flow ---
                // After forwarding, we *should* ideally wait for an update
                // from Fcitx via receive_update(). The current loop structure
                // is too simple for this request-response cycle combined with
                // potential asynchronous updates from Fcitx.
                // For now, we just loop back and check receive_update again.
                // A simulated update might occur in receive_update based on
                // the forwarded key in a more advanced placeholder.

                // We might re-render immediately or wait for Fcitx update
                // terminal.render(app_state)?;
            }
            Ok(None) => {
                // EOF reached (e.g., Ctrl+D)
                println!("EOF received.");
                break;
            }
            Err(e) => {
                eprintln!("Error reading terminal input: {}", e);
                break; // Exit loop on error
            }
        }
    }

    Ok(())
}
