use crate::error::FepError;
use crate::state::AppState;
use crossterm::{
    cursor::{self, MoveLeft, MoveRight, MoveToColumn}, // カーソル移動コマンドを追加
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, EventStream},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Stdout, Write};
use futures_util::{Stream, StreamExt, TryStreamExt};
// unicode-width クレートを追加するとより正確な幅計算が可能 (今回は使わない)
// use unicode_width::UnicodeWidthStr;

pub struct Terminal {
    stdout: Stdout,
}

impl Terminal {
    /// Creates a new Terminal handler and enters raw mode. (同期のまま)
    pub fn new() -> Result<Self, FepError> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()
            .map_err(|e| FepError::TerminalSetup(format!("Failed to enable raw mode: {}", e)))?;
        execute!(stdout, cursor::Hide)
            .map_err(|e| FepError::TerminalSetup(format!("Failed to hide cursor: {}", e)))?;
        Ok(Terminal { stdout })
    }

    /// Returns an async stream of terminal key events.
    pub fn key_event_stream(&self) -> impl Stream<Item = Result<KeyEvent, FepError>> + Send + Unpin {
        // EventStream は Unpin を実装している
        EventStream::new()
            .filter_map(|maybe_event| async {
                match maybe_event {
                    Ok(Event::Key(key_event)) => Some(Ok(key_event)),
                    Ok(_) => None, // Ignore non-key events
                    Err(e) => Some(Err(FepError::Io(e))),
                }
            })
            // Add Send + Unpin bounds if needed by the caller context (like tokio::select!)
            // EventStream itself should be Send + Unpin if the underlying reader is.
    }

    /// Renders the state, positioning the cursor correctly.
    pub fn render(&mut self, state: &AppState) -> Result<(), FepError> {
        // 1. カーソルを行頭に移動し、行をクリア
        execute!(
            self.stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::FromCursorDown),
        )?;

        let mut current_column: u16 = 0;

        // 2. 未確定文字列 (Preedit) を表示
        if !state.preedit_string.is_empty() {
            // スタイルを設定して未確定文字列を出力
            execute!(
                self.stdout,
                SetAttribute(Attribute::Underlined),
                Print(&state.preedit_string),
                SetAttribute(Attribute::Reset)
            )?;
            // 文字列の表示幅を計算 (簡易的に文字数を使用)
            // 注意: 全角文字や特殊文字を考慮すると不正確。
            // 正確な計算には unicode-width クレートなどが推奨される。
            current_column = state.preedit_string.chars().count() as u16; // 表示後のカーソル位置 (文字数)

            // カーソルを未確定文字列内の指定位置に移動
            // state.preedit_cursor_pos は文字単位の位置
            let target_cursor_char_pos = state.preedit_cursor_pos;
            let chars_to_move_left = current_column.saturating_sub(target_cursor_char_pos as u16);

            if chars_to_move_left > 0 {
                execute!(self.stdout, MoveLeft(chars_to_move_left))?;
            }
            current_column = target_cursor_char_pos as u16; // カーソル位置を更新
        }

        // 3. 確定文字列 (Commit) を表示
        if !state.commit_string.is_empty() {
            // 未確定文字列が表示されていた場合は、一度行頭に戻ってクリアし直すか、
            // 現在の位置から出力するかを決める。
            // ここでは、未確定文字列がクリアされている前提 (AppState でクリアされる) で、
            // 現在のカーソル位置 (通常は行頭) から確定文字列を出力する。
            execute!(self.stdout, Print(&state.commit_string))?;
            // 確定文字列の表示幅を計算 (簡易的に文字数)
            current_column += state.commit_string.chars().count() as u16;
            // カーソルは確定文字列の直後に移動しているはず
        }

        // 4. 最終的なカーソル位置を再確認 (特に何も表示されなかった場合など)
        // execute!(self.stdout, cursor::MoveToColumn(current_column))?; // 必要に応じて

        // 5. Flush output buffer
        self.stdout.flush().map_err(FepError::Io)?;

        Ok(())
    }

    /// Cleans up the terminal (同期のまま).
    fn cleanup(&mut self) {
        let _ = execute!(self.stdout, cursor::Show);
        let _ = terminal::disable_raw_mode();
        // Note: Adding a newline might interfere if there was an error message printed after cleanup
        // println!("\nTerminal cleanup completed.");
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.cleanup();
    }
}
