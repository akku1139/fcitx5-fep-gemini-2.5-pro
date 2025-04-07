use crate::error::FepError;
use crate::state::AppState; // AppState を参照
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers}, // イベント関連を追加
    execute, // execute! マクロ用
    style::{Attribute, Print, SetAttribute}, // スタイル関連を追加
    terminal::{self, Clear, ClearType}, // ターミナル制御関連を追加
};
use std::io::{self, Stdout, Write}; // Stdout と Write をインポート
use std::time::Duration; // Duration を追加

pub struct Terminal {
    stdout: Stdout, // 標準出力を保持
}

impl Terminal {
    /// Creates a new Terminal handler and enters raw mode.
    pub fn new() -> Result<Self, FepError> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()
            .map_err(|e| FepError::TerminalSetup(format!("Failed to enable raw mode: {}", e)))?;
        execute!(stdout, cursor::Hide) // カーソルを隠す (オプション)
            .map_err(|e| FepError::TerminalSetup(format!("Failed to hide cursor: {}", e)))?;
        Ok(Terminal { stdout })
    }

    /// Reads a key event from the terminal (stdin).
    /// Uses polling to be non-blocking for a short duration.
    /// Returns Ok(Some(KeyEvent)) if a key is pressed, Ok(None) if timeout occurs, Err on error.
    pub fn read_input(&mut self) -> Result<Option<KeyEvent>, FepError> {
        // Poll for events with a short timeout (e.g., 100ms)
        // Adjust timeout as needed for responsiveness vs CPU usage trade-off.
        if event::poll(Duration::from_millis(100))
            .map_err(|e| FepError::Io(e))? {
            // If an event is available, read it
            match event::read().map_err(|e| FepError::Io(e))? {
                Event::Key(key_event) => {
                    // Handle Ctrl+C explicitly for exiting raw mode gracefully
                    if key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        Err(FepError::TerminalSetup("Ctrl+C pressed".to_string())) // Use a specific error or signal
                    } else {
                        Ok(Some(key_event))
                    }
                }
                // Handle other events like Resize if necessary
                _ => Ok(None), // Ignore non-key events for now
            }
        } else {
            // Timeout expired, no event
            Ok(None)
        }
    }

    /// Clears the current line and displays the application state using crossterm.
    pub fn render(&mut self, state: &AppState) -> Result<(), FepError> {
        execute!(
            self.stdout,
            // 1. Move cursor to the beginning of the line
            cursor::MoveToColumn(0),
            // 2. Clear the line from cursor to the end
            Clear(ClearType::FromCursorDown), // Or ClearType::CurrentLine
        )?;

        // 3. Display the preedit string with underline
        if !state.preedit_string.is_empty() {
            execute!(
                self.stdout,
                SetAttribute(Attribute::Underlined),
                Print(&state.preedit_string),
                SetAttribute(Attribute::Reset) // Reset attributes
            )?;
            // TODO: Add cursor positioning within or after preedit based on Fcitx state
        }

        // 4. Display the commit string (directly output)
        // In a real app, commit string is usually "typed" by the FEP,
        // replacing the FEP's own display. Here we just print it after preedit.
        if !state.commit_string.is_empty() {
            // Ensure commit string starts on the same line if preedit was cleared
            // or after the preedit if it exists. This logic might need refinement.
            execute!(self.stdout, Print(&state.commit_string))?;
        }

        // 5. Flush output buffer
        self.stdout.flush().map_err(FepError::Io)?;

        Ok(())
    }

    /// Cleans up the terminal by disabling raw mode and showing the cursor.
    fn cleanup(&mut self) {
        // It's good practice to try cleaning up, even if errors occur.
        let _ = execute!(self.stdout, cursor::Show); // Show cursor again
        let _ = terminal::disable_raw_mode(); // Disable raw mode
        println!("\nTerminal cleanup completed."); // Add newline after raw mode exit
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.cleanup();
    }
}
