use crate::fcitx::FormattedText; // FormattedText をインポート

/// Represents updates received from Fcitx5.
#[derive(Debug, Clone)]
pub enum FcitxUpdate {
    // UpdatePreedit にカーソル位置 (バイト単位) を含める
    UpdatePreedit {
        text: String,
        cursor_pos: i32, // Fcitx から送られてくるカーソル位置 (バイト単位が多い)
        // 必要であればフォーマット情報も保持
        // formatted_text: Vec<FormattedText>,
    },
    CommitString(String),
    // Add other update types like candidate list etc. if needed
}

/// Holds the current state of the FEP.
#[derive(Debug)]
pub struct AppState {
    pub preedit_string: String,
    pub preedit_cursor_pos: usize, // カーソル位置 (文字単位で保持するのが Rust では扱いやすい)
    pub commit_string: String,
    // Add other state variables as needed
}

impl AppState {
    /// Creates a new initial state.
    pub fn new() -> Self {
        AppState {
            preedit_string: String::new(),
            preedit_cursor_pos: 0,
            commit_string: String::new(),
        }
    }

    /// Updates the state based on an update received from Fcitx.
    pub fn apply_update(&mut self, update: FcitxUpdate) {
        // Clear previous commit string before applying new updates
        self.commit_string.clear();

        match update {
            FcitxUpdate::UpdatePreedit { text, cursor_pos, .. } => {
                self.preedit_string = text;
                // Fcitx のカーソル位置はバイト単位の場合が多い。
                // Rust の文字列操作は文字単位 (char) や UTF-8 バイト境界で行うため、
                // バイト位置から文字位置への変換が必要になる場合がある。
                // ここでは、単純に usize にキャストするが、マルチバイト文字が多い場合は注意が必要。
                // 正確な文字位置が必要な場合は、バイトインデックスを使って文字境界を探す。
                // 例: self.preedit_string.char_indices().take_while(|(idx, _)| *idx < cursor_pos as usize).count();
                // 簡単のため、ここでは cursor_pos が文字位置を指していると仮定するか、
                // バイト位置として扱い、描画時に調整する。今回は usize にキャストしておく。
                self.preedit_cursor_pos = cursor_pos.max(0) as usize;
                // カーソル位置が文字列長を超える場合を考慮
                if self.preedit_cursor_pos > self.preedit_string.chars().count() {
                     self.preedit_cursor_pos = self.preedit_string.chars().count();
                }

                println!(
                    "State Updated: Preedit='{}', Cursor={}",
                    self.preedit_string, self.preedit_cursor_pos
                );
            }
            FcitxUpdate::CommitString(commit) => {
                self.commit_string = commit;
                // Commit 発生時は Preedit をクリアし、カーソル位置もリセット
                self.preedit_string.clear();
                self.preedit_cursor_pos = 0;
                println!("State Updated: Commit='{}'", self.commit_string);
            }
        }
    }
}
