use crate::error::FepError;
use crate::fcitx::FcitxClient;
use crate::state::{AppState, FcitxUpdate}; // FcitxUpdate をインポート
use crate::terminal::Terminal;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
// use std::{thread, time::Duration}; // 不要になる
use futures_util::{StreamExt, TryStreamExt}; // StreamExt, TryStreamExt
use tokio::select; // tokio::select! マクロ

// --- keysyms と masks モジュール (変更なし) ---
mod keysyms {
    // ... (省略) ...
    pub const XK_BackSpace: u32 = 0xff08;
    pub const XK_Tab: u32 = 0xff09;
    pub const XK_Return: u32 = 0xff0d;
    pub const XK_Escape: u32 = 0xff1b;
    pub const XK_Left: u32 = 0xff51;
    pub const XK_Up: u32 = 0xff52;
    pub const XK_Right: u32 = 0xff53;
    pub const XK_Down: u32 = 0xff54;
    pub const XK_Delete: u32 = 0xffff;
    pub const XK_space: u32 = 0x0020;
    pub const XK_0: u32 = 0x0030;
    pub const XK_9: u32 = 0x0039;
    pub const XK_A: u32 = 0x0041;
    pub const XK_Z: u32 = 0x005a;
    pub const XK_a: u32 = 0x0061;
    pub const XK_z: u32 = 0x007a;
}
mod masks {
    // ... (省略) ...
    pub const ShiftMask: u32 = 1 << 0;
    pub const LockMask: u32 = 1 << 1;
    pub const ControlMask: u32 = 1 << 2;
    pub const Mod1Mask: u32 = 1 << 3;
    pub const Mod2Mask: u32 = 1 << 4;
    pub const Mod3Mask: u32 = 1 << 5;
    pub const Mod4Mask: u32 = 1 << 6;
    pub const Mod5Mask: u32 = 1 << 7;
}

/// Maps crossterm KeyEvent to Fcitx compatible (keysym, keycode, state). (変更なし)
fn map_key_event_to_fcitx(key_event: &KeyEvent) -> Option<(u32, u32, u32)> {
    // ... (実装は省略 - 前回のコードと同じ) ...
    let mut state = 0u32;
    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
        state |= masks::ShiftMask;
    }
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        state |= masks::ControlMask;
    }
    if key_event.modifiers.contains(KeyModifiers::ALT) {
        state |= masks::Mod1Mask;
    }

    let keysym = match key_event.code {
        KeyCode::Char(c) => c as u32,
        KeyCode::Backspace => keysyms::XK_BackSpace,
        KeyCode::Enter => keysyms::XK_Return,
        KeyCode::Left => keysyms::XK_Left,
        KeyCode::Right => keysyms::XK_Right,
        KeyCode::Up => keysyms::XK_Up,
        KeyCode::Down => keysyms::XK_Down,
        KeyCode::Tab => keysyms::XK_Tab,
        KeyCode::Delete => keysyms::XK_Delete,
        KeyCode::Esc => keysyms::XK_Escape,
        _ => return None,
    };
    let keycode = 0;
    Some((keysym, keycode, state))
}

/// Runs the main async event loop.
pub async fn run_event_loop<'a>(
    terminal: &'a mut Terminal, // Borrow terminal mutably
    fcitx_client: &'a mut FcitxClient<'a>, // Borrow client mutably
    app_state: &'a mut AppState, // Borrow state mutably
) -> Result<(), FepError> {
    println!("Entering async event loop...");

    // Get the async streams
    let mut key_stream = terminal.key_event_stream();
    let mut fcitx_updates = fcitx_client.receive_updates().await?; // Get the merged stream

    // Initial render
    terminal.render(app_state)?;

    loop {
        select! {
            // Bias select slightly towards terminal input if needed? Default is random.
            // biased; // Uncomment for bias

            // Wait for the next terminal key event
            maybe_key_event = key_stream.next() => {
                match maybe_key_event {
                    Some(Ok(key_event)) => {
                        // Handle Ctrl+C for graceful shutdown (if not handled by tokio::signal)
                        if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                             println!("Ctrl+C detected in stream. Exiting loop.");
                             break; // Exit loop
                        }

                        println!("Terminal Event: {:?}", key_event);
                        if let Some((keysym, keycode, state)) = map_key_event_to_fcitx(&key_event) {
                            // Forward key event (async)
                            match fcitx_client.forward_key_event(keysym, keycode, state, false).await {
                                Ok(handled) => {
                                    if !handled {
                                        println!("Key not handled by Fcitx.");
                                    }
                                    // Render might be triggered by fcitx update below
                                }
                                Err(e) => {
                                    eprintln!("Error forwarding key event: {}", e);
                                    return Err(e); // Propagate error
                                }
                            }
                        } else {
                            println!("Key ignored (no mapping).");
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Error reading terminal input stream: {}", e);
                        return Err(e); // Propagate error
                    }
                    None => {
                        // Terminal stream ended? Should not happen normally unless stdin closes.
                        println!("Terminal input stream ended.");
                        break; // Exit loop
                    }
                }
            }

            // Wait for the next Fcitx update event
            maybe_fcitx_update = fcitx_updates.next() => {
                 match maybe_fcitx_update {
                    Some(Ok(update)) => {
                        println!("Fcitx Update: {:?}", update);
                        app_state.apply_update(update);
                        terminal.render(app_state)?; // Re-render after state update
                    }
                    Some(Err(e)) => {
                        eprintln!("Error receiving Fcitx update stream: {}", e);
                        return Err(e); // Propagate error
                    }
                    None => {
                        // Fcitx update stream ended? Might happen if connection drops.
                        println!("Fcitx update stream ended.");
                        // Maybe try to reconnect or exit? For now, exit.
                        return Err(FepError::FcitxConnection("Fcitx update stream unexpectedly ended".to_string()));
                    }
                 }
            }

            // Add other branches to select! if needed (e.g., timers, other signals)

            // Default branch (optional, runs if no other branch is ready immediately)
            // default => {
            //     // Can be used for periodic tasks, but select! efficiently sleeps
            // }

            // Complete branch (runs after any branch completes one await)
            // complete => {
            //     // Useful for cleanup or checks after any event
            // }
        } // end select!
    } // end loop

    println!("Exiting async event loop normally.");
    Ok(())
}
