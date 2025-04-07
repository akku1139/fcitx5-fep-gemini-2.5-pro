use crate::error::FepError;
use crate::fcitx::FcitxClient;
use crate::state::AppState;
use crate::terminal::Terminal;

// crossterm イベントをインポート
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{thread, time::Duration};

// X11 Keysym definitions (一部) - 必要に応じて追加
// 参考: https://www.cl.cam.ac.uk/~mgk25/ucs/keysymdef.h
// または /usr/include/X11/keysymdef.h
mod keysyms {
    pub const XK_BackSpace: u32 = 0xff08;
    pub const XK_Tab: u32 = 0xff09;
    pub const XK_Return: u32 = 0xff0d; // Enter key
    pub const XK_Escape: u32 = 0xff1b;
    pub const XK_Left: u32 = 0xff51;
    pub const XK_Up: u32 = 0xff52;
    pub const XK_Right: u32 = 0xff53;
    pub const XK_Down: u32 = 0xff54;
    pub const XK_Delete: u32 = 0xffff;
    // Basic Latin characters (assuming ASCII/UTF-8 mapping)
    pub const XK_space: u32 = 0x0020;
    // ... punctuation ...
    pub const XK_0: u32 = 0x0030;
    pub const XK_9: u32 = 0x0039;
    pub const XK_A: u32 = 0x0041;
    pub const XK_Z: u32 = 0x005a;
    pub const XK_a: u32 = 0x0061;
    pub const XK_z: u32 = 0x007a;
    // Add more as needed...
}

// Modifier masks (X11)
mod masks {
    pub const ShiftMask: u32 = 1 << 0;
    pub const LockMask: u32 = 1 << 1; // Caps Lock
    pub const ControlMask: u32 = 1 << 2;
    pub const Mod1Mask: u32 = 1 << 3; // Alt
    pub const Mod2Mask: u32 = 1 << 4; // Num Lock
    pub const Mod3Mask: u32 = 1 << 5;
    pub const Mod4Mask: u32 = 1 << 6; // Super/Win
    pub const Mod5Mask: u32 = 1 << 7;
}


/// Maps crossterm KeyEvent to Fcitx compatible (keysym, keycode, state).
/// Returns None if the key should not be forwarded.
fn map_key_event_to_fcitx(key_event: &KeyEvent) -> Option<(u32, u32, u32)> {
    let mut state = 0u32;
    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
        state |= masks::ShiftMask;
    }
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        state |= masks::ControlMask;
    }
    if key_event.modifiers.contains(KeyModifiers::ALT) {
        state |= masks::Mod1Mask; // Alt is typically Mod1
    }
    // Note: SUPER/META (Mod4Mask) might need explicit handling if needed.
    // Note: CapsLock (LockMask) and NumLock (Mod2Mask) state might be needed for accuracy.

    let keysym = match key_event.code {
        KeyCode::Char(c) => {
            // Basic mapping: assumes the character code is close to the X11 keysym value
            // This works reasonably well for ASCII letters and numbers.
            // Punctuation and non-ASCII require a more complex mapping table.
            // crossterm already gives uppercase when Shift is pressed for letters.
            c as u32
        }
        KeyCode::Backspace => keysyms::XK_BackSpace,
        KeyCode::Enter => keysyms::XK_Return,
        KeyCode::Left => keysyms::XK_Left,
        KeyCode::Right => keysyms::XK_Right,
        KeyCode::Up => keysyms::XK_Up,
        KeyCode::Down => keysyms::XK_Down,
        KeyCode::Tab => keysyms::XK_Tab,
        KeyCode::Delete => keysyms::XK_Delete,
        KeyCode::Esc => keysyms::XK_Escape,
        // Add mappings for F-keys, Home, End, PageUp/Down etc. if needed
        // KeyCode::F(num) => 0xffbe + num as u32, // Example for F-keys
        _ => {
            // Ignore other keys for now (Insert, Menu, Function keys, etc.)
            return None;
        }
    };

    // Use 0 as a placeholder keycode. Fcitx often relies on keysym + state.
    let keycode = 0;

    Some((keysym, keycode, state))
}


/// Runs the main event loop.
pub fn run_event_loop(
    terminal: &mut Terminal,
    fcitx_client: &mut FcitxClient,
    app_state: &mut AppState,
) -> Result<(), FepError> {
    println!("Entering event loop. Press Ctrl+C to exit.");

    // Initial render
    terminal.render(app_state)?;

    loop {
        let mut event_processed = false;

        // 1. Check for Terminal Input (non-blocking poll)
        match terminal.read_input() {
            Ok(Some(key_event)) => {
                event_processed = true;
                println!("Terminal Event: {:?}", key_event); // Log the received key event

                // Map KeyEvent to fcitx parameters
                if let Some((keysym, keycode, state)) = map_key_event_to_fcitx(&key_event) {
                    // Forward the key event to Fcitx
                    match fcitx_client.forward_key_event(keysym, keycode, state, false) { // Assuming press only (false)
                        Ok(handled) => {
                            if !handled {
                                // Fcitx didn't handle the key.
                                // A real FEP might insert the character directly or pass it through.
                                // For now, we do nothing.
                                println!("Key not handled by Fcitx.");
                            }
                            // We expect Fcitx to send signals (CommitString, UpdatePreedit)
                            // which will be handled below. Re-rendering might be delayed
                            // until an update is received. Or render immediately? Let's wait.
                        }
                        Err(e) => {
                            eprintln!("Error forwarding key event: {}", e);
                            // Decide how to handle error (e.g., break loop)
                            return Err(e); // Exit loop on error
                        }
                    }
                } else {
                    println!("Key ignored (no mapping).");
                }
                // Re-render immediately after processing local key? Maybe not needed if waiting for fcitx update.
                // terminal.render(app_state)?;
            }
            Ok(None) => {
                // No terminal input event within the poll timeout
            }
            Err(FepError::TerminalSetup(msg)) if msg == "Ctrl+C pressed" => {
                println!("Ctrl+C detected. Exiting.");
                break; // Exit loop gracefully
            }
            Err(e) => {
                eprintln!("Error reading terminal input: {}", e);
                return Err(e); // Exit loop on error
            }
        }

        // 2. Check for Fcitx D-Bus Updates (non-blocking poll)
        match fcitx_client.receive_update() {
            Ok(Some(update)) => {
                event_processed = true;
                // Apply the update from Fcitx to our state
                app_state.apply_update(update);
                // Re-render the terminal display with the new state
                terminal.render(app_state)?;
            }
            Ok(None) => {
                // No update from Fcitx
            }
            Err(e) => {
                eprintln!("Error receiving Fcitx update: {}", e);
                // Decide how to handle error (e.g., break loop)
                return Err(e); // Exit loop on error
            }
        }

        // 3. Avoid busy-waiting if no events were processed
        if !event_processed {
            // Sleep for a very short duration if nothing happened,
            // prevents tight loop consuming 100% CPU if poll timeouts are very short.
            thread::sleep(Duration::from_millis(10)); // Adjust sleep time as needed
        }
    } // end loop

    Ok(())
}
