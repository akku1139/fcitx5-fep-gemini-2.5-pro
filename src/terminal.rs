// src/terminal.rs
// Handles terminal setup, raw mode, rendering, and provides an async event stream.

use crate::error::FepError;
use crate::state::AppState;
use crossterm::{
    cursor::{self, MoveLeft, MoveToColumn}, // Import cursor commands
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, EventStream}, // Use EventStream
    execute, // For executing terminal commands
    style::{Attribute, Print, SetAttribute}, // For styling output
    terminal::{self, Clear, ClearType}, // For terminal control (raw mode, clear)
};
use std::io::{self, Stdout, Write};
use futures_util::{Stream, StreamExt}; // Stream and StreamExt for async stream handling

// Optional: For accurate character width calculation (not used here to minimize deps)
// use unicode_width::UnicodeWidthStr;

/// Manages terminal state and interaction.
pub struct Terminal {
    stdout: Stdout, // Handle to standard output
}

impl Terminal {
    /// Creates a new Terminal handler, enters raw mode, and hides the cursor.
    /// This setup is synchronous.
    pub fn new() -> Result<Self, FepError> {
        let mut stdout = io::stdout();
        // Enter raw mode to process key events directly
        terminal::enable_raw_mode()
            .map_err(|e| FepError::TerminalSetup(format!("Failed to enable raw mode: {}", e)))?;
        // Hide the cursor for cleaner FEP display
        execute!(stdout, cursor::Hide)
            .map_err(|e| FepError::TerminalSetup(format!("Failed to hide cursor: {}", e)))?;
        Ok(Terminal { stdout })
    }

    /// Returns an asynchronous stream of terminal key events.
    /// Filters out non-key events.
    pub fn key_event_stream(&self) -> impl Stream<Item = Result<KeyEvent, FepError>> + Send + Unpin {
        EventStream::new() // Create a stream of terminal events
            .filter_map(|maybe_event| async { // Process each event asynchronously
                match maybe_event {
                    // If it's a key event, yield it as Ok(KeyEvent)
                    Ok(Event::Key(key_event)) => Some(Ok(key_event)),
                    // Ignore other event types (Mouse, Resize, etc.)
                    Ok(_) => None,
                    // If there's an error reading the event, yield it as Err(FepError)
                    Err(e) => Some(Err(FepError::Io(e))),
                }
            })
    }


    /// Renders the current application state (preedit, commit) to the terminal.
    /// Handles cursor positioning based on preedit state. This is synchronous.
    pub fn render(&mut self, state: &AppState) -> Result<(), FepError> {
        // --- Prepare Rendering Commands ---

        // 1. Move cursor to the beginning of the line and clear it
        execute!(
            self.stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::FromCursorDown), // Clear from cursor to end of screen might be safer
                                             // Clear(ClearType::CurrentLine), // Or just clear the current line
        )?;

        let mut current_cursor_col: u16 = 0; // Track estimated cursor column

        // 2. Render Preedit String (if any)
        if !state.preedit_string.is_empty() {
            // Apply underline style and print the preedit text
            execute!(
                self.stdout,
                SetAttribute(Attribute::Underlined),
                Print(&state.preedit_string),
                SetAttribute(Attribute::Reset) // Reset style immediately after
            )?;

            // Calculate the display width of the preedit string.
            // WARNING: Using chars().count() is NOT accurate for CJK or wide characters.
            // For accurate width, use a crate like `unicode_width`.
            // let preedit_display_width = UnicodeWidthStr::width(state.preedit_string.as_str());
            let preedit_display_width = state.preedit_string.chars().count(); // Simple char count approximation

            // Calculate the display width up to the cursor position (character-based).
            let cursor_target_char_index = state.preedit_cursor_pos;
            let width_to_cursor = state.preedit_string
                .chars()
                .take(cursor_target_char_index)
                .count(); // Simple char count approximation

            // Move the cursor back from the end of the printed string to the target position.
            let chars_to_move_left = preedit_display_width.saturating_sub(width_to_cursor);
            if chars_to_move_left > 0 {
                execute!(self.stdout, MoveLeft(chars_to_move_left as u16))?;
            }
            current_cursor_col = width_to_cursor as u16; // Update estimated cursor column
        }

        // 3. Render Commit String (if any)
        // This typically happens after preedit is cleared by AppState update.
        if !state.commit_string.is_empty() {
            // Print the commit string at the current cursor position (usually column 0 after preedit clear)
            execute!(self.stdout, Print(&state.commit_string))?;

            // Update estimated cursor column after printing commit string
            // WARNING: Again, using chars().count() is not accurate for width.
            let commit_display_width = state.commit_string.chars().count();
            current_cursor_col += commit_display_width as u16;
        }

        // 4. Ensure the cursor is positioned correctly (optional final adjustment)
        // execute!(self.stdout, cursor::MoveToColumn(current_cursor_col))?;

        // 5. Flush stdout to make changes visible
        self.stdout.flush().map_err(FepError::Io)?;

        Ok(())
    }

    /// Cleans up the terminal state (synchronous).
    /// Disables raw mode and shows the cursor. Called automatically on Drop.
    fn cleanup(&mut self) {
        // Ignore errors during cleanup, as we're likely exiting anyway.
        let _ = execute!(self.stdout, cursor::Show); // Restore cursor visibility
        let _ = terminal::disable_raw_mode(); // Exit raw mode
        // Printing here might interfere with final error messages from main
        // println!("\nTerminal cleanup completed.");
    }
}

// Drop implementation ensures cleanup happens when Terminal goes out of scope.
impl Drop for Terminal {
    fn drop(&mut self) {
        self.cleanup();
    }
}
