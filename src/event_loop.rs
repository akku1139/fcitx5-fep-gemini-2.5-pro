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

    pub const XK_Z: u32 = 0x005a;
    pub const XK_bracketleft: u32 = 0x005b; // [
    pub const XK_backslash: u32 = 0x005c; // \
    pub const XK_bracketright: u32 = 0x005d; // ]
    pub const XK_asciicircum: u32 = 0x005e; // ^
    pub const XK_underscore: u32 = 0x005f; // _
    pub const XK_grave: u32 = 0x0060; // `
    pub const XK_a: u32 = 0x0061;

    pub const XK_z: u32 = 0x007a;
    pub const XK_braceleft: u32 = 0x007b; // {
    pub const XK_bar: u32 = 0x007c; // |
    pub const XK_braceright: u32 = 0x007d; // }
    pub const XK_asciitilde: u32 = 0x007e; // ~
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

/// Maps crossterm KeyEvent to Fcitx compatible (keysym, keycode, state).
fn map_key_event_to_fcitx(key_event: &KeyEvent) -> Option<(u32, u32, u32)> {
    let mut state = 0u32;
    // crossterm のモディファイアから X11 の state mask を生成
    if key_event.modifiers.contains(KeyModifiers::SHIFT) {
        state |= masks::ShiftMask;
    }
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        state |= masks::ControlMask;
    }
    if key_event.modifiers.contains(KeyModifiers::ALT) {
        state |= masks::Mod1Mask;
    }
    // 他のモディファイア (Super, Hyper, Meta, CapsLock, NumLock) は省略

    // crossterm の KeyCode から X11 の keysym を決定
    let keysym = match key_event.code {
        // 文字キー: crossterm は Shift を考慮した文字を返すことが多い
        KeyCode::Char(c) => match c {
            ' ' => keysyms::XK_space,
            '!' => keysyms::XK_exclam,
            '"' => keysyms::XK_quotedbl,
            '#' => keysyms::XK_numbersign,
            '$' => keysyms::XK_dollar,
            '%' => keysyms::XK_percent,
            '&' => keysyms::XK_ampersand,
            '\'' => keysyms::XK_apostrophe,
            '(' => keysyms::XK_parenleft,
            ')' => keysyms::XK_parenright,
            '*' => keysyms::XK_asterisk,
            '+' => keysyms::XK_plus,
            ',' => keysyms::XK_comma,
            '-' => keysyms::XK_minus,
            '.' => keysyms::XK_period,
            '/' => keysyms::XK_slash,
            '0'..='9' => keysyms::XK_0 + (c as u32 - '0' as u32),
            ':' => keysyms::XK_colon,
            ';' => keysyms::XK_semicolon,
            '<' => keysyms::XK_less,
            '=' => keysyms::XK_equal,
            '>' => keysyms::XK_greater,
            '?' => keysyms::XK_question,
            '@' => keysyms::XK_at,
            'A'..='Z' => keysyms::XK_A + (c as u32 - 'A' as u32),
            '[' => keysyms::XK_bracketleft,
            '\\' => keysyms::XK_backslash,
            ']' => keysyms::XK_bracketright,
            '^' => keysyms::XK_asciicircum,
            '_' => keysyms::XK_underscore,
            '`' => keysyms::XK_grave,
            'a'..='z' => keysyms::XK_a + (c as u32 - 'a' as u32),
            '{' => keysyms::XK_braceleft,
            '|' => keysyms::XK_bar,
            '}' => keysyms::XK_braceright,
            '~' => keysyms::XK_asciitilde,
            // 上記以外 (非ASCII文字など) はそのまま Unicode コードポイントを使う
            // Fcitx がこれを解釈できるかは Fcitx 側の実装による
            _ => c as u32,
        },
        // 特殊キー
        KeyCode::Backspace => keysyms::XK_BackSpace,
        KeyCode::Enter => keysyms::XK_Return,
        KeyCode::Left => keysyms::XK_Left,
        KeyCode::Right => keysyms::XK_Right,
        KeyCode::Up => keysyms::XK_Up,
        KeyCode::Down => keysyms::XK_Down,
        KeyCode::Tab => keysyms::XK_Tab,
        KeyCode::Delete => keysyms::XK_Delete,
        KeyCode::Esc => keysyms::XK_Escape,
        // 他のキー (Home, End, Insert, F1-F12 etc.) は必要なら追加
        _ => return None, // マッピングできないキーは無視
    };

    // keycode は 0 (プレースホルダー)
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
