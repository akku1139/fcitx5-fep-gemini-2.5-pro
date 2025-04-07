// src/fcitx.rs
// Handles asynchronous communication with the Fcitx5 daemon via D-Bus using zbus.

use crate::error::FepError;
use crate::state::FcitxUpdate;
use std::collections::HashMap;
use zbus::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Type, Value}; // Use OwnedObjectPath
use zbus_macros::{proxy, DeserializeInto, Serialize};
use futures_util::{Stream, StreamExt}; // Stream and StreamExt for handling signals
use tokio_stream::wrappers::SignalStream; // Wrapper for zbus signals

// --- D-Bus Constants ---
const FCITX5_SERVICE: &str = "org.fcitx.Fcitx5";
const FCITX5_IFACE_CONTROLLER: &str = "org.fcitx.Fcitx.Controller1";
const FCITX5_IFACE_IC: &str = "org.fcitx.Fcitx.InputContext1";
const FCITX5_PATH: &str = "/org/fcitx/Fcitx5";

// --- D-Bus Proxy Definitions ---

#[proxy(
    interface = "org.fcitx.Fcitx.Controller1",
    default_service = "org.fcitx.Fcitx5",
    default_path = "/org/fcitx/Fcitx5"
)]
trait FcitxController {
    /// Creates an input context for an application.
    #[zbus(name = "CreateInputContext")]
    async fn create_input_context(
        &self,
        args: &HashMap<&str, zbus::zvariant::Value<'_>>, // e.g., {"program": "my_app"}
    ) -> zbus::Result<(OwnedObjectPath, u32)>; // Returns IC path and capabilities
}

#[proxy(interface = "org.fcitx.Fcitx.InputContext1")]
trait FcitxInputContext {
    /// Processes a key event. Returns true if handled by Fcitx.
    #[zbus(name = "ProcessKeyEvent")]
    async fn process_key_event(
        &self,
        keysym: u32,
        keycode: u32,
        state: u32,
        is_release: bool,
        time: u32,
    ) -> zbus::Result<bool>;

    /// Notifies Fcitx that the input context gained focus.
    #[zbus(name = "FocusIn")]
    async fn focus_in(&self) -> zbus::Result<()>;

    /// Notifies Fcitx that the input context lost focus.
    #[zbus(name = "FocusOut")]
    async fn focus_out(&self) -> zbus::Result<()>;

    /// Resets the input context state.
    #[zbus(name = "Reset")]
    async fn reset(&self) -> zbus::Result<()>;

    /// Sets the position of the cursor rectangle (for candidate window placement).
    #[zbus(name = "SetCursorRect")]
    async fn set_cursor_rect(&self, x: i32, y: i32, w: i32, h: i32) -> zbus::Result<()>;

    // --- Signals ---

    /// Signal emitted when text should be committed.
    #[zbus(signal)]
    async fn commit_string(&self, str: String) -> zbus::Result<()>;

    /// Signal emitted when the preedit text changes (with formatting).
    #[zbus(signal)]
    async fn update_formatted_preedit(&self, text: Vec<FormattedText>, cursor_pos: i32) -> zbus::Result<()>;

    /// Signal emitted when surrounding text should be deleted.
    // #[zbus(signal)]
    // async fn delete_surrounding_text(&self, offset: i32, n_chars: u32) -> zbus::Result<()>;
}

/// Represents a segment of formatted preedit text.
#[derive(DeserializeInto, Type, Debug, Clone)]
pub struct FormattedText {
    text: String,
    format: i32, // Corresponds to FcitxFormattedPreeditFormat enum (e.g., 0=None, 1=Underline)
}

// --- Fcitx Client Implementation (Async) ---

pub struct FcitxClient<'a> {
    connection: Connection, // Async Connection
    ic_proxy: Option<FcitxInputContextProxy<'a>>, // Async Proxy for the Input Context
    ic_path: Option<OwnedObjectPath>, // Store the path for signal matching if needed (proxy handles it)
}

impl<'a> FcitxClient<'a> {
    /// Establishes an async connection to Fcitx5 and creates an input context.
    pub async fn connect() -> Result<Self, FepError> {
        println!("Connecting to Fcitx5 via D-Bus (async)...");
        let connection = Connection::session().await?; // Use ? for From<zbus::Error>
        println!("D-Bus session connection established.");

        let controller_proxy = FcitxControllerProxy::new(&connection).await?;
        println!("Fcitx controller proxy created.");

        // Prepare arguments for CreateInputContext
        let mut args = HashMap::new();
        // Use a unique name for the application if possible
        args.insert("program", Value::from("fcitx5-fep-rust").into());
        // Optionally add display, capabilities etc.
        // args.insert("display", Value::from(std::env::var("DISPLAY").unwrap_or(":0".to_string())));

        println!("Calling CreateInputContext (async)...");
        let (ic_path, _ic_caps) = controller_proxy.create_input_context(&args).await?;
        println!("Input Context created at path: {}", ic_path);

        // Create the async proxy for the newly created Input Context
        let ic_proxy = FcitxInputContextProxy::builder(&connection)
            .path(ic_path.clone())? // Build proxy for the specific path
            .build().await?;
        println!("Input context proxy created.");

        let mut client = FcitxClient {
            connection,
            ic_proxy: Some(ic_proxy),
            ic_path: Some(ic_path), // Store path if needed elsewhere, though proxy knows its path
        };

        // Activate the input context by sending FocusIn
        client.focus_in().await?;
        println!("Input context focused.");

        Ok(client)
    }

    /// Returns a combined stream of relevant Fcitx updates (CommitString, UpdateFormattedPreedit).
    /// The stream yields Result<FcitxUpdate, FepError>.
    pub async fn receive_updates(&self) -> Result<impl Stream<Item = Result<FcitxUpdate, FepError>> + '_, FepError> {
        let proxy = self.ic_proxy.as_ref().ok_or_else(|| FepError::FcitxConnection("Input context proxy not available for signals".to_string()))?;

        // Create streams for individual signals using the proxy methods
        let commit_signal_stream = proxy.receive_commit_string().await?;
        let preedit_signal_stream = proxy.receive_update_formatted_preedit().await?;

        // Map the signal arguments (contained in Result<SignalArgsType, zbus::Error>) to our FcitxUpdate enum
        let commit_stream = commit_signal_stream.map(|args_result| {
             args_result
                 .map(|args| FcitxUpdate::CommitString(args.str)) // Access args by name defined in signal method
                 .map_err(FepError::from) // Convert zbus::Error to FepError
        });

        let preedit_stream = preedit_signal_stream.map(|args_result| {
             args_result.map(|args| {
                 // args is (Vec<FormattedText>, i32)
                 let text = args.text.into_iter().map(|s| s.text).collect::<String>();
                 let cursor_pos = args.cursor_pos; // Cursor position in bytes
                 println!("Raw Preedit Signal: text='{}', cursor_pos={}", text, cursor_pos);
                 FcitxUpdate::UpdatePreedit { text, cursor_pos }
             })
             .map_err(FepError::from) // Convert zbus::Error to FepError
        });

        // Merge the two streams into a single stream using tokio_stream::StreamExt::merge
        Ok(tokio_stream::StreamExt::merge(commit_stream, preedit_stream))
    }

    /// Sends FocusIn signal to the input context (async).
    pub async fn focus_in(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_in().await?;
        }
        Ok(())
    }

     /// Sends FocusOut signal to the input context (async).
    pub async fn focus_out(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_out().await?;
        }
        Ok(())
    }

    /// Sends Reset signal to the input context (async).
     pub async fn reset(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.reset().await?;
        }
        Ok(())
    }

    /// Sends a key event to Fcitx5 using provided keysym, keycode, and state (async).
    pub async fn forward_key_event(
        &mut self,
        keysym: u32,
        keycode: u32, // Placeholder (0) is often acceptable
        state: u32,   // Modifier state mask
        is_release: bool, // Currently assuming false (press only)
    ) -> Result<bool, FepError> {
        let proxy = self.ic_proxy.as_mut().ok_or_else(|| FepError::FcitxConnection("Input context proxy not available".to_string()))?;
        let time = 0; // Event timestamp, 0 is usually fine

        println!(
            "Forwarding key to Fcitx5 (async): keysym=0x{:x}, keycode={}, state={}, release={}",
            keysym, keycode, state, is_release
        );

        // Call the D-Bus method asynchronously
        match proxy.process_key_event(keysym, keycode, state, is_release, time).await {
            Ok(handled) => {
                println!("Fcitx handled key event: {}", handled);
                Ok(handled)
            },
            Err(e) => {
                 eprintln!("Error forwarding key event: {}", e);
                 Err(FepError::from(e)) // Convert zbus::Error
            }
        }
    }

    /// Performs asynchronous cleanup before dropping if necessary.
    /// Currently only sends FocusOut.
    pub async fn disconnect(&mut self) {
        println!("Disconnecting from Fcitx5 (async)...");
        if let Some(proxy) = self.ic_proxy.as_mut() {
            // Try to send FocusOut, ignore error if it fails during shutdown
            let _ = proxy.focus_out().await;
        }
        // Clear the proxy and path
        self.ic_proxy = None;
        self.ic_path = None;
        println!("Fcitx5 client disconnected.");
    }
}

// Drop implementation for automatic cleanup (cannot be async)
impl<'a> Drop for FcitxClient<'a> {
    fn drop(&mut self) {
        // If async cleanup (like FocusOut) is critical, it should be called explicitly
        // via `disconnect().await` before dropping the client.
        // Dropping the `zbus::Connection` handles closing the D-Bus connection.
        println!("FcitxClient dropped, D-Bus connection will be closed.");
    }
}
