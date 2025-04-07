use crate::error::FepError;
use crate::state::FcitxUpdate;
use std::collections::HashMap;
// use std::convert::TryFrom; // 不要になる可能性
// use std::time::Duration; // 不要になる
use zbus::{Connection, Proxy}; // blocking を削除
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value}; // Owned* 型を使うことが多い
use zbus_macros::{proxy, DeserializeInto, Serialize};
use futures_util::{Stream, StreamExt}; // Stream と StreamExt をインポート
use tokio_stream::wrappers::SignalStream; // SignalStream をインポート

// --- 定数と Proxy 定義 (変更なし) ---
const FCITX5_SERVICE: &str = "org.fcitx.Fcitx5";
const FCITX5_IFACE_CONTROLLER: &str = "org.fcitx.Fcitx.Controller1";
const FCITX5_IFACE_IC: &str = "org.fcitx.Fcitx.InputContext1";
const FCITX5_PATH: &str = "/org/fcitx/Fcitx5";

#[proxy(
    interface = "org.fcitx.Fcitx.Controller1",
    default_service = "org.fcitx.Fcitx5",
    default_path = "/org/fcitx/Fcitx5"
)]
trait FcitxController {
    /// CreateInputContext method (async)
    #[zbus(name = "CreateInputContext")] // 明示的に名前を指定
    async fn create_input_context(
        &self,
        args: &HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<(OwnedObjectPath, u32)>; // OwnedObjectPath を使用
}

#[proxy(interface = "org.fcitx.Fcitx.InputContext1")]
trait FcitxInputContext {
    /// ProcessKeyEvent method (async)
    #[zbus(name = "ProcessKeyEvent")]
    async fn process_key_event(
        &self,
        keysym: u32,
        keycode: u32,
        state: u32,
        is_release: bool,
        time: u32,
    ) -> zbus::Result<bool>;

    /// FocusIn method (async)
    #[zbus(name = "FocusIn")]
    async fn focus_in(&self) -> zbus::Result<()>;

    /// FocusOut method (async)
    #[zbus(name = "FocusOut")]
    async fn focus_out(&self) -> zbus::Result<()>;

    /// Reset method (async)
    #[zbus(name = "Reset")]
    async fn reset(&self) -> zbus::Result<()>;

    /// SetCursorRect method (async, example)
    #[zbus(name = "SetCursorRect")]
    async fn set_cursor_rect(&self, x: i32, y: i32, w: i32, h: i32) -> zbus::Result<()>;

    // --- Signals ---
    // receive_commit_string のようなメソッドで Stream を取得する

    /// CommitString signal receiver
    #[zbus(signal)]
    async fn commit_string(&self, str: String) -> zbus::Result<()>;

    /// UpdateFormattedPreedit signal receiver
    #[zbus(signal)]
    async fn update_formatted_preedit(&self, text: Vec<FormattedText>, cursor_pos: i32) -> zbus::Result<()>;

    // DeleteSurroundingText signal (example)
    // #[zbus(signal)]
    // async fn delete_surrounding_text(&self, offset: i32, n_chars: u32) -> zbus::Result<()>;
}

#[derive(DeserializeInto, Type, Debug, Clone)]
pub struct FormattedText {
    text: String,
    format: i32,
}

// --- Fcitx Client Implementation (Async) ---

pub struct FcitxClient<'a> {
    connection: Connection, // Async Connection
    // controller_proxy: FcitxControllerProxy<'a>, // Async Proxy type
    ic_proxy: Option<FcitxInputContextProxy<'a>>, // Async Proxy
    ic_path: Option<OwnedObjectPath>, // Owned path
}

// We need a helper struct to hold the streams because the proxy reference cannot be held across awaits easily
struct FcitxSignalStreams<'a> {
     commit_stream: SignalStream<'a, String>,
     preedit_stream: SignalStream<'a, (Vec<FormattedText>, i32)>,
     // Add other signal streams if needed
}


impl<'a> FcitxClient<'a> {
    /// Establishes an async connection and creates an input context.
    pub async fn connect() -> Result<Self, FepError> {
        println!("Connecting to Fcitx5 via D-Bus (async)...");
        let connection = Connection::session().await // await for async connection
            .map_err(|e| FepError::FcitxConnection(e.to_string()))?;
        println!("D-Bus session connection established.");

        let controller_proxy = FcitxControllerProxy::new(&connection).await // await proxy creation
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create controller proxy: {}", e)))?;
        println!("Fcitx controller proxy created.");

        let mut args = HashMap::new();
        args.insert("program", Value::from("fep-rust-example-async").into());

        println!("Calling CreateInputContext (async)...");
        let (ic_path, _ic_caps) = controller_proxy.create_input_context(&args).await // await method call
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create input context: {}", e)))?;
        println!("Input Context created at path: {}", ic_path);

        // Create the async proxy for the Input Context
        let ic_proxy = FcitxInputContextProxy::builder(&connection)
            .path(ic_path.clone())? // Use clone of OwnedObjectPath
            .build().await // await async build
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create IC proxy: {}", e)))?;
        println!("Input context proxy created.");

        let mut client = FcitxClient {
            connection,
            ic_proxy: Some(ic_proxy),
            ic_path: Some(ic_path),
        };

        // Activate the input context (async)
        client.focus_in().await?;
        println!("Input context focused.");

        Ok(client)
    }

    /// Returns a combined stream of relevant Fcitx updates.
    pub async fn receive_updates(&self) -> Result<impl Stream<Item = Result<FcitxUpdate, FepError>> + '_, FepError> {
        let proxy = self.ic_proxy.as_ref().ok_or_else(|| FepError::FcitxConnection("Input context proxy not available for signals".to_string()))?;

        let commit_signal_stream = proxy.receive_commit_string().await
             .map_err(|e| FepError::FcitxConnection(format!("Failed to receive CommitString signal: {}", e)))?;
        let preedit_signal_stream = proxy.receive_update_formatted_preedit().await
             .map_err(|e| FepError::FcitxConnection(format!("Failed to receive UpdateFormattedPreedit signal: {}", e)))?;

        let commit_stream = commit_signal_stream.map(|args_result| {
             args_result
                 .map(|args| FcitxUpdate::CommitString(args.str))
                 .map_err(|e| FepError::FcitxConnection(format!("CommitString signal error: {}", e)))
        });

        let preedit_stream = preedit_signal_stream.map(|args_result| {
             args_result.map(|args| {
                 // args は (Vec<FormattedText>, i32) 型
                 let text = args.text.into_iter().map(|s| s.text).collect::<String>();
                 let cursor_pos = args.cursor_pos; // カーソル位置を取得
                 println!("Raw Preedit Signal: text='{}', cursor_pos={}", text, cursor_pos);
                 // 新しい FcitxUpdate::UpdatePreedit を使用
                 FcitxUpdate::UpdatePreedit { text, cursor_pos }
             })
             .map_err(|e| FepError::FcitxConnection(format!("UpdateFormattedPreedit signal error: {}", e)))
        });

        Ok(tokio_stream::StreamExt::merge(commit_stream, preedit_stream))
    }

    /// Sends FocusIn signal (async).
    pub async fn focus_in(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_in().await.map_err(|e| FepError::FcitxConnection(format!("FocusIn failed: {}", e)))?;
        }
        Ok(())
    }

     /// Sends FocusOut signal (async).
    pub async fn focus_out(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_out().await.map_err(|e| FepError::FcitxConnection(format!("FocusOut failed: {}", e)))?;
        }
        Ok(())
    }

    /// Sends Reset signal (async).
     pub async fn reset(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.reset().await.map_err(|e| FepError::FcitxConnection(format!("Reset failed: {}", e)))?;
        }
        Ok(())
    }

    /// Sends a key event to Fcitx5 (async).
    pub async fn forward_key_event(
        &mut self,
        keysym: u32,
        keycode: u32,
        state: u32,
        is_release: bool,
    ) -> Result<bool, FepError> {
        let proxy = self.ic_proxy.as_mut().ok_or_else(|| FepError::FcitxConnection("Input context proxy not available".to_string()))?;
        let time = 0;

        println!(
            "Forwarding key to Fcitx5 (async): keysym=0x{:x}, keycode={}, state={}, release={}",
            keysym, keycode, state, is_release
        );

        match proxy.process_key_event(keysym, keycode, state, is_release, time).await { // await the async call
            Ok(handled) => {
                println!("Fcitx handled key event: {}", handled);
                Ok(handled)
            },
            Err(e) => {
                 eprintln!("Error forwarding key event: {}", e);
                 Err(FepError::FcitxConnection(format!("ProcessKeyEvent failed: {}", e)))
            }
        }
    }

    /// Disconnects (async cleanup if needed).
    pub async fn disconnect(&mut self) {
        println!("Disconnecting from Fcitx5 (async)...");
        if let Some(proxy) = self.ic_proxy.as_mut() {
            if let Err(e) = proxy.focus_out().await { // await focus_out
                eprintln!("Error sending FocusOut on disconnect: {}", e);
            }
        }
        self.ic_proxy = None;
        self.ic_path = None;
        println!("Fcitx5 disconnected (connection will close on drop).");
    }
}

// Implement Drop for async cleanup if necessary, though connection drop might suffice
impl<'a> Drop for FcitxClient<'a> {
    fn drop(&mut self) {
        // Note: Drop cannot be async. If async cleanup is strictly required,
        // it must be called explicitly before dropping (e.g., client.disconnect().await).
        // For simple cases, dropping the connection might be enough.
        println!("FcitxClient dropped.");
    }
}
