// Defines the application state, including preedit string, commit string, etc.

/// Represents updates received from Fcitx5.
#[derive(Debug, Clone)]
pub enum FcitxUpdate {
    UpdatePreedit(String), // The new preedit string (potentially combined from formatted segments)
    CommitString(String),  // The string to be committed to the application
    // TODO: Add other update types like cursor position, candidate list etc.
    // TODO: Potentially use a richer type than String for preedit to keep formatting.
}

/// Holds the current state of the FEP.
#[derive(Debug)]
pub struct AppState {
    pub preedit_string: String,
    pub commit_string: String,
    // Add other state variables like cursor position in preedit,
    // candidate window visibility, etc.
}

impl AppState {
    /// Creates a new initial state.
    pub fn new() -> Self {
        AppState {
            preedit_string: String::new(),
            commit_string: String::new(),
        }
    }

    /// Updates the state based on an update received from Fcitx.
    pub fn apply_update(&mut self, update: FcitxUpdate) {
        // Clear previous commit string before applying new updates
        self.commit_string.clear();

        match update {
            FcitxUpdate::UpdatePreedit(preedit) => {
                self.preedit_string = preedit;
                println!("State Updated: Preedit='{}'", self.preedit_string);
            }
            FcitxUpdate::CommitString(commit) => {
                self.commit_string = commit;
                // Typically, preedit is cleared on commit
                self.preedit_string.clear();
                println!("State Updated: Commit='{}'", self.commit_string);
            } // Handle other update types
        }
    }
}
