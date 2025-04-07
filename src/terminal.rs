use crate::error::FepError;
use crate::state::AppState;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, EventStream}, // EventStream を追加
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Stdout, Write};
// use std::time::Duration; // 不要になる
use futures_util::{Stream, StreamExt, TryStreamExt}; // Stream と TryStreamExt をインポート

pub struct Terminal {
    stdout: Stdout,
    // EventStream を保持しない方が良いかもしれない (ライフタイムの問題)
    // 代わりに stream を返すメソッドを提供する
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


    /// Renders the state (同期のまま).
    pub fn render(&mut self, state: &AppState) -> Result<(), FepError> {
        execute!(
            self.stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::FromCursorDown),
        )?;

        if !state.preedit_string.is_empty() {
            execute!(
                self.stdout,
                SetAttribute(Attribute::Underlined),
                Print(&state.preedit_string),
                SetAttribute(Attribute::Reset)
            )?;
        }

        if !state.commit_string.is_empty() {
            execute!(self.stdout, Print(&state.commit_string))?;
        }

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
