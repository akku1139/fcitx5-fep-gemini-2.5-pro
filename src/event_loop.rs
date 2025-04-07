// src/event_loop.rs
// The main asynchronous event loop using tokio::select!

use crate::error::FepError;
use crate::fcitx::FcitxClient;
use crate::state::{AppState, FcitxUpdate}; // Import FcitxUpdate
use crate::terminal::Terminal;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures_util::{StreamExt}; // StreamExt for stream methods like next()
use tokio::select; // The core macro for concurrent async operations

// --- X11 Keysym Definitions ---
// Provides constants for common key symbols used by Fcitx.
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
    pub const XK_space: u32 = 0x0020;
    pub const XK_exclam: u32 = 0x0021; // !
    pub const XK_quotedbl: u32 = 0x0022; // "
    pub const XK_numbersign: u32 = 0x0023; // #
    pub const XK_dollar: u32 = 0x0024; // $
    pub const XK_percent: u32 = 0x0025; // %
    pub const XK_ampersand: u32 = 0x0026; // &
    pub const XK_apostrophe: u32 = 0x0027; // '
    pub const XK_parenleft: u32 = 0x0028; // (
    pub const XK_parenright: u32 = 0x0029; // )
    pub const XK_asterisk: u32 = 0x002a; // *
    pub const XK_plus: u32 = 0x002b; // +
    pub const XK_comma: u32 = 0x002c; // ,
    pub const XK_minus: u32 = 0x002d; // -
    pub const XK_period: u32 = 0x002e; // .
    pub const XK_slash: u32 = 0x002f; // /
    pub const XK_0: u32 = 0x0030;
    pub const XK_1: u32 = 0x0031;
    pub const XK_2: u32 = 0x0032;
    pub const XK_3: u32 = 0x0033;
    pub const XK_4: u32 = 0x0034;
    pub const XK_5: u32 = 0x0035;
    pub const XK_6: u32 = 0x0036;
    pub const XK_7: u32 = 0x0037;
    pub const XK_8: u32 = 0x0038;
    pub const XK_9: u32 = 0x0039;
    pub const XK_colon: u32 = 0x003a; // :
    pub const XK_semicolon: u32 = 0x003b; // ;
    pub const XK_less: u32 = 0x003c; // <
    pub const XK_equal: u32 = 0x003d; // =
    pub const XK_greater: u32 = 0x003e; // >
    pub const XK_question: u32 = 0x003f; // ?
    pub const XK_at: u32 = 0x0040; // @
    pub const XK_A: u32 = 0x0041;
    pub const XK_B: u32 = 0x0042;
    pub const XK_C: u32 = 0x0043;
    pub const XK_D: u32 = 0x0044;
    pub const XK_E: u32 = 0x0045;
    pub const XK_F: u32 = 0x0046;
    pub const XK_G: u32 = 0x0047;
    pub const XK_H: u32 = 0x0048;
    pub const XK_I: u32 = 0x0049;
    pub const XK_J: u32 = 0x004a;
    pub const XK_K: u32 = 0x004b;
    pub const XK_L: u32 = 0x004c;
    pub const XK_M: u32 = 0x004d;
    pub const XK_N: u32 = 0x004e;
    pub const XK_O: u32 = 0x004f;
    pub const XK_P: u32 = 0x0050;
    pub const XK_Q: u32 = 0x0051;
    pub const XK_R: u32 = 0x0052;
    pub const XK_S: u32 = 0x0053;
    pub const XK_T: u32 = 0x0054;
    pub const XK_U: u32 = 0x0055;
    pub const XK_V: u32 = 0x0056;
    pub const XK_W: u32 = 0x0057;
    pub const XK_X: u32 = 0x0058;
    pub const XK_Y: u32 = 0x0059;
    pub const XK_Z: u32 = 0x005a;
    pub const XK_bracketleft: u32 = 0x005b; // [
    pub const XK_backslash: u32 = 0x005c; // \ -> Note: Often requires Shift on some layouts
    pub const XK_bracketright: u32 = 0x005d; // ]
    pub const XK_asciicircum: u32 = 0x005e; // ^
    pub const XK_underscore: u32 = 0x005f; // _
    pub const XK_grave: u32 = 0x0060; // `
    pub const XK_a: u32 = 0x0061;
    pub const XK_b: u32 = 0x0062;
    pub const XK_c: u32 = 0x0063;
    pub const XK_d: u32 = 0x0064;
    pub const XK_e: u32 = 0x0065;
    pub const XK_f: u32 = 0x0066;
    pub const XK_g: u32 = 0x0067;
    pub const XK_h: u32 = 0x0068;
    pub const XK_i: u32 = 0x0069;
    pub const XK_j: u32 = 0x006a;
    pub const XK_k: u32 = 0x006b;
    pub const XK_l: u32 = 0x006c;
    pub const XK_m: u32 = 0x006d;
    pub const XK_n: u32 = 0x006e;
    pub const XK_o: u32 = 0x006f;
    pub const XK_p: u32 = 0x0070;
    pub const XK_q: u32 = 0x0071;
    pub const XK_r: u32 = 0x0072;
    pub const XK_s: u32 = 0x0073;
    pub const XK_t: u32 = 0x0074;
    pub const XK_u: u32 = 0x0075;
    pub const XK_v: u32 = 0x0076;
    pub const XK_w: u32 = 0x0077;
    pub const XK_x: u32 = 0x0078;
    pub const XK_y: u32 = 0x0079;
    pub const XK_z: u32 = 0x007a;
    pub const XK_braceleft: u32 = 0x007b; // {
    pub const XK_bar: u32 = 0x007c; // |
    pub const XK_braceright: u32 = 0x007d; // }
    pub const XK_asciitilde: u32 = 0x007e; // ~
}

// --- X11 Modifier Masks ---
// Provides constants for modifier key states.
mod masks {
    pub const ShiftMask: u32 = 1 << 0;
    pub const LockMask: u32 = 1 << 1; // Caps Lock
    pub const ControlMask: u32 = 1 << 2;
    pub const Mod1Mask: u32 = 1 << 3; // Alt key (usually)
    pub const Mod2Mask: u32 = 1 << 4; // Num Lock key (usually)
    pub const Mod3Mask: u32 = 1 << 5; // Often unused
    pub const Mod4Mask: u32 = 1 << 6; // Super/Win key (usually)
    pub const Mod5Mask: u32 = 1 << 7; // Often ISO_Level3_Shift (AltGr)
}

/// Maps a crossterm KeyEvent to Fcitx compatible (keysym, keycode, state).
/// Returns None if the key event should not be forwarded to Fcitx.
fn map_key_event_to_fcitx(key_event: &KeyEvent) -> Option<(u32, u32, u32)> {
    let mut state = 0u32;
    // Map crossterm modifiers to X11 state mask
    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
        state |= masks::ShiftMask;
    }
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        state |= masks::ControlMask;
    }
    if key_event.modifiers.contains(KeyModifiers::ALT) {
        state |= masks::Mod1Mask; // Assuming Alt is Mod1
    }
    // Note: Handling SUPER (Mod4Mask), AltGr (Mod5Mask), CapsLock, NumLock
    // would require more complex state tracking or platform APIs.

    // Map crossterm KeyCode to X11 Keysym
    let keysym = match key_event.code {
        // --- Character Keys ---
        // crossterm provides the character considering Shift state,
        // so we map directly to the corresponding keysym constant.
        KeyCode::Char(c) => match c {
            ' ' => keysyms::XK_space,
            '!' => keysyms::XK_exclam, '"' => keysyms::XK_quotedbl, '#' => keysyms::XK_numbersign,
            '$' => keysyms::XK_dollar, '%' => keysyms::XK_percent, '&' => keysyms::XK_ampersand,
            '\'' => keysyms::XK_apostrophe, '(' => keysyms::XK_parenleft, ')' => keysyms::XK_parenright,
            '*' => keysyms::XK_asterisk, '+' => keysyms::XK_plus, ',' => keysyms::XK_comma,
            '-' => keysyms::XK_minus, '.' => keysyms::XK_period, '/' => keysyms::XK_slash,
            '0' => keysyms::XK_0, '1' => keysyms::XK_1, '2' => keysyms::XK_2, '3' => keysyms::XK_3,
            '4' => keysyms::XK_4, '5' => keysyms::XK_5, '6' => keysyms::XK_6, '7' => keysyms::XK_7,
            '8' => keysyms::XK_8, '9' => keysyms::XK_9,
            ':' => keysyms::XK_colon, ';' => keysyms::XK_semicolon, '<' => keysyms::XK_less,
            '=' => keysyms::XK_equal, '>' => keysyms::XK_greater, '?' => keysyms::XK_question,
            '@' => keysyms::XK_at,
            'A' => keysyms::XK_A, 'B' => keysyms::XK_B, 'C' => keysyms::XK_C, 'D' => keysyms::XK_D,
            'E' => keysyms::XK_E, 'F' => keysyms::XK_F, 'G' => keysyms::XK_G, 'H' => keysyms::XK_H,
            'I' => keysyms::XK_I, 'J' => keysyms::XK_J, 'K' => keysyms::XK_K, 'L' => keysyms::XK_L,
            'M' => keysyms::XK_M, 'N' => keysyms::XK_N, 'O' => keysyms::XK_O, 'P' => keysyms::XK_P,
            'Q' => keysyms::XK_Q, 'R' => keysyms::XK_R, 'S' => keysyms::XK_S, 'T' => keysyms::XK_T,
            'U' => keysyms::XK_U, 'V' => keysyms::XK_V, 'W' => keysyms::XK_W, 'X' => keysyms::XK_X,
            'Y' => keysyms::XK_Y, 'Z' => keysyms::XK_Z,
            '[' => keysyms::XK_bracketleft, '\\' => keysyms::XK_backslash, ']' => keysyms::XK_bracketright,
            '^' => keysyms::XK_asciicircum, '_' => keysyms::XK_underscore, '`' => keysyms::XK_grave,
            'a' => keysyms::XK_a, 'b' => keysyms::XK_b, 'c' => keysyms::XK_c, 'd' => keysyms::XK_d,
            'e' => keysyms::XK_e, 'f' => keysyms::XK_f, 'g' => keysyms::XK_g, 'h' => keysyms::XK_h,
            'i' => keysyms::XK_i, 'j' => keysyms::XK_j, 'k' => keysyms::XK_k, 'l' => keysyms::XK_l,
            'm' => keysyms::XK_m, 'n' => keysyms::XK_n, 'o' => keysyms::XK_o, 'p' => keysyms::XK_p,
            'q' => keysyms::XK_q, 'r' => keysyms::XK_r, 's' => keysyms::XK_s, 't' => keysyms::XK_t,
            'u' => keysyms::XK_u, 'v' => keysyms::XK_v, 'w' => keysyms::XK_w, 'x' => keysyms::XK_x,
            'y' => keysyms::XK_y, 'z' => keysyms::XK_z,
            '{' => keysyms::XK_braceleft, '|' => keysyms::XK_bar, '}' => keysyms::XK_braceright,
            '~' => keysyms::XK_asciitilde,
            // For other characters (e.g., non-ASCII), pass the Unicode codepoint directly.
            // Fcitx might interpret this based on layout or internal state.
            _ => c as u32,
        },

        // --- Special Keys ---
        KeyCode::Backspace => keysyms::XK_BackSpace,
        KeyCode::Enter => keysyms::XK_Return,
        KeyCode::Left => keysyms::XK_Left,
        KeyCode::Right => keysyms::XK_Right,
        KeyCode::Up => keysyms::XK_Up,
        KeyCode::Down => keysyms::XK_Down,
        KeyCode::Tab => keysyms::XK_Tab,
        KeyCode::Delete => keysyms::XK_Delete,
        KeyCode::Esc => keysyms::XK_Escape,
        // Add Home, End, PageUp, PageDown, Insert, F1-F12 etc. if needed
        // KeyCode::Home => keysyms::XK_Home,
        // KeyCode::End => keysyms::XK_End,
        // KeyCode::PageUp => keysyms::XK_Page_Up,
        // KeyCode::PageDown => keysyms::XK_Page_Down,
        // KeyCode::Insert => keysyms::XK_Insert,
        // KeyCode::F(n) => keysyms::XK_F1 + (n as u32 - 1),

        // Ignore keys not explicitly handled
        _ => return None,
    };

    // Use 0 as a placeholder keycode. Fcitx generally works well with keysym + state.
    let keycode = 0;

    Some((keysym, keycode, state))
}


/// Runs the main asynchronous event loop, handling terminal input and Fcitx D-Bus signals.
pub async fn run_event_loop<'a>(
    terminal: &'a mut Terminal, // Borrow terminal mutably
    fcitx_client: &'a mut FcitxClient<'a>, // Borrow client mutably
    app_state: &'a mut AppState, // Borrow state mutably
) -> Result<(), FepError> {
    println!("Entering async event loop...");

    // Get the asynchronous streams for terminal events and Fcitx updates
    let mut key_stream = terminal.key_event_stream();
    let mut fcitx_updates = fcitx_client.receive_updates().await?; // Setup signal listeners

    // Perform an initial render of the empty state
    terminal.render(app_state)?;

    // Main loop: concurrently wait for events from either stream
    loop {
        select! {
            // Biasing can prioritize one stream slightly if needed, but usually not necessary.
            // biased;

            // Branch 1: Handle Terminal Input Events
            maybe_key_event = key_stream.next() => {
                match maybe_key_event {
                    Some(Ok(key_event)) => {
                        // Check for Ctrl+C specifically (if not handled by tokio::signal)
                        // This provides an in-loop exit mechanism.
                        if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                             println!("Ctrl+C detected in terminal stream. Exiting loop.");
                             break; // Exit the event loop
                        }

                        println!("Terminal Event: {:?}", key_event); // Log received event

                        // Map the crossterm event to Fcitx parameters
                        if let Some((keysym, keycode, state)) = map_key_event_to_fcitx(&key_event) {
                            // Forward the mapped event to Fcitx asynchronously
                            match fcitx_client.forward_key_event(keysym, keycode, state, false).await { // Assuming key press (is_release = false)
                                Ok(handled) => {
                                    if !handled {
                                        // Fcitx did not consume the event.
                                        // A more advanced FEP might insert the character directly here,
                                        // but that requires careful state management. We ignore it for now.
                                        println!("Key event not handled by Fcitx.");
                                    }
                                    // We expect Fcitx to potentially send back updates (preedit/commit)
                                    // via the fcitx_updates stream, which will trigger rendering.
                                }
                                Err(e) => {
                                    // Log and propagate the error if forwarding fails
                                    eprintln!("Error forwarding key event to Fcitx: {}", e);
                                    return Err(e);
                                }
                            }
                        } else {
                            // Key was not mapped (e.g., unsupported special key)
                            println!("Key ignored (no mapping to Fcitx parameters).");
                        }
                    }
                    Some(Err(e)) => {
                        // Error reading from the terminal stream
                        eprintln!("Error reading terminal input stream: {}", e);
                        return Err(e); // Propagate the error
                    }
                    None => {
                        // The terminal input stream has ended (e.g., stdin closed).
                        println!("Terminal input stream ended.");
                        break; // Exit the event loop
                    }
                }
            }

            // Branch 2: Handle Fcitx D-Bus Signal Updates
            maybe_fcitx_update = fcitx_updates.next() => {
                 match maybe_fcitx_update {
                    Some(Ok(update)) => {
                        // Received an update (CommitString or UpdatePreedit) from Fcitx
                        println!("Fcitx Update Received: {:?}", update);
                        // Apply the update to the application state
                        app_state.apply_update(update);
                        // Re-render the terminal to reflect the new state
                        terminal.render(app_state)?;
                    }
                    Some(Err(e)) => {
                        // Error receiving or processing an Fcitx update signal
                        eprintln!("Error receiving Fcitx update stream: {}", e);
                        return Err(e); // Propagate the error
                    }
                    None => {
                        // The Fcitx update stream ended unexpectedly.
                        // This might indicate the Fcitx connection was lost.
                        println!("Fcitx update stream ended unexpectedly.");
                        // Return an error indicating the connection issue.
                        return Err(FepError::FcitxConnection("Fcitx update stream unexpectedly ended".to_string()));
                    }
                 }
            }
        } // end select!
    } // end loop

    println!("Exiting async event loop normally.");
    Ok(())
}

