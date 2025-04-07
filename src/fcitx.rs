use crate::error::FepError;
use crate::state::FcitxUpdate; // state.rs も後で調整が必要
use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedValue, Type, Value};
use zbus_macros::{proxy, DeserializeInto, Serialize}; // proxyマクロを追加

// Fcitx D-Bus 定数
const FCITX5_SERVICE: &str = "org.fcitx.Fcitx5";
const FCITX5_IFACE_CONTROLLER: &str = "org.fcitx.Fcitx.Controller1";
const FCITX5_IFACE_IC: &str = "org.fcitx.Fcitx.InputContext1";
const FCITX5_PATH: &str = "/org/fcitx/Fcitx5";

// --- D-Bus Proxy Definitions ---
// zbus_macros::proxy を使ってインターフェースを定義すると便利

#[proxy(
    interface = "org.fcitx.Fcitx.Controller1",
    default_service = "org.fcitx.Fcitx5",
    default_path = "/org/fcitx/Fcitx5"
)]
trait FcitxController {
    /// CreateInputContext method
    /// Returns the object path of the new input context and its capabilities.
    fn create_input_context(
        &self,
        args: &HashMap<&str, zbus::zvariant::Value<'_>>, // e.g., {"program": "my_app", "display": ":0"}
    ) -> zbus::Result<(ObjectPath<'static>, u32)>;
}

// InputContext 用の Proxy も定義
#[proxy(interface = "org.fcitx.Fcitx.InputContext1")]
trait FcitxInputContext {
    /// ProcessKeyEvent method
    /// Returns true if the key event was handled by the input method.
    fn process_key_event(
        &self,
        keysym: u32,
        keycode: u32,
        state: u32,
        is_release: bool,
        time: u32, // Usually 0 is fine
    ) -> zbus::Result<bool>;

    /// FocusIn method
    fn focus_in(&self) -> zbus::Result<()>;

    /// FocusOut method
    fn focus_out(&self) -> zbus::Result<()>;

    /// Reset method
    fn reset(&self) -> zbus::Result<()>;

    /// SetCursorRect method (example)
    fn set_cursor_rect(&self, x: i32, y: i32, w: i32, h: i32) -> zbus::Result<()>;

    // --- Signals to listen for ---

    /// CommitString signal
    #[zbus(signal)]
    fn commit_string(&self, str: String) -> zbus::Result<()>;

    /// UpdateFormattedPreedit signal
    /// Sends an array of (text_segment, format_type)
    #[zbus(signal)]
    fn update_formatted_preedit(&self, text: Vec<FormattedText>, cursor_pos: i32) -> zbus::Result<()>;

    // DeleteSurroundingText signal (example)
    // #[zbus(signal)]
    // fn delete_surrounding_text(&self, offset: i32, n_chars: u32) -> zbus::Result<()>;
}

/// Represents a segment of formatted preedit text.
/// `zvariant::Type` と `serde::Deserialize` が必要
#[derive(DeserializeInto, Type, Debug, Clone)]
pub struct FormattedText {
    text: String,
    format: i32, // Corresponds to FcitxFormattedPreeditFormat enum
}


// --- Fcitx Client Implementation ---

pub struct FcitxClient<'a> {
    connection: Connection,
    // controller_proxy: FcitxControllerProxyBlocking<'a>, // Use generated proxy type
    ic_proxy: Option<FcitxInputContextProxyBlocking<'a>>, // Proxy for the specific Input Context
    ic_path: Option<ObjectPath<'static>>, // Store the path for signal matching
}

impl<'a> FcitxClient<'a> {
    /// Establishes a connection to the Fcitx5 daemon and creates an input context.
    pub fn connect() -> Result<Self, FepError> {
        println!("Connecting to Fcitx5 via D-Bus...");
        let connection = Connection::session().map_err(|e| FepError::FcitxConnection(e.to_string()))?;
        println!("D-Bus session connection established.");

        // Create a proxy for the main controller
        let controller_proxy = FcitxControllerProxyBlocking::new(&connection)
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create controller proxy: {}", e)))?;
        println!("Fcitx controller proxy created.");

        // Prepare arguments for CreateInputContext
        // TODO: Get actual display if needed, handle errors better
        let mut args = HashMap::new();
        args.insert("program", Value::from("fep-rust-example").into());
        // args.insert("display", Value::from(std::env::var("DISPLAY").unwrap_or(":0".to_string())));

        println!("Calling CreateInputContext...");
        let (ic_path, _ic_caps) = controller_proxy.create_input_context(&args)
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create input context: {}", e)))?;
        println!("Input Context created at path: {}", ic_path);

        // Create a proxy for the newly created Input Context
        // We need to build the proxy manually here as the path is dynamic
        let ic_proxy = Proxy::builder(&connection)
            .interface(FCITX5_IFACE_IC)?
            .path(ic_path.clone())?
            .destination(FCITX5_SERVICE)?
            .build_blocking() // Build the blocking proxy
            .map_err(|e| FepError::FcitxConnection(format!("Failed to create IC proxy: {}", e)))?;
        println!("Input context proxy created.");

        let mut client = FcitxClient {
            connection,
            // controller_proxy,
            ic_proxy: Some(ic_proxy),
            ic_path: Some(ic_path),
        };

        // Activate the input context
        client.focus_in()?;
        println!("Input context focused.");

        Ok(client)
    }

    /// Sends FocusIn signal to the input context.
    pub fn focus_in(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_in().map_err(|e| FepError::FcitxConnection(format!("FocusIn failed: {}", e)))?;
        }
        Ok(())
    }

     /// Sends FocusOut signal to the input context.
    pub fn focus_out(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.focus_out().map_err(|e| FepError::FcitxConnection(format!("FocusOut failed: {}", e)))?;
        }
        Ok(())
    }

    /// Sends Reset signal to the input context.
     pub fn reset(&mut self) -> Result<(), FepError> {
        if let Some(proxy) = self.ic_proxy.as_mut() {
            proxy.reset().map_err(|e| FepError::FcitxConnection(format!("Reset failed: {}", e)))?;
        }
        Ok(())
    }


    /// Sends a key event to Fcitx5.
    /// NOTE: Mapping string input to keysym/keycode/state is complex and not fully implemented here.
    pub fn forward_key_event(&mut self, key_input: &str) -> Result<bool, FepError> {
        let proxy = self.ic_proxy.as_mut().ok_or_else(|| FepError::FcitxConnection("Input context proxy not available".to_string()))?;

        // --- VERY SIMPLIFIED key mapping ---
        // A real implementation needs a robust mapping from terminal key events
        // (including modifiers like Shift, Ctrl, Alt) to X11/Wayland keysyms, keycodes, and state masks.
        // This often requires libraries or complex platform-specific code.
        let (keysym, keycode, state) = match key_input {
            // Example: Map 'a'
            "a" => (0x0061, 38, 0), // keysym, keycode (example), state (no modifiers)
            // Example: Map 'A' (Shift + a)
            "A" => (0x0041, 38, 1), // keysym, keycode, state (ShiftMask = 1)
             // Example: Map Enter
            "\n" | "\r" | "Enter" => (0xff0d, 36, 0), // XK_Return
             // Example: Map Backspace
            "Backspace" => (0xff08, 22, 0), // XK_BackSpace
            // Add more mappings as needed...
            _ => {
                // Basic printable ASCII mapping (highly inaccurate for non-US layouts)
                if key_input.len() == 1 && key_input.chars().next().unwrap().is_ascii() {
                    let c = key_input.chars().next().unwrap();
                    // This is a HACK: using ASCII value as keysym, placeholder keycode/state
                    (c as u32, 0, 0)
                } else {
                    println!("Warning: Unhandled key input for Fcitx: '{}'", key_input);
                    return Ok(false); // Don't forward unhandled keys
                }
            }
        };
        let is_release = false; // Assuming key press only for now
        let time = 0; // Typically okay for Fcitx

        println!(
            "Forwarding key to Fcitx5: keysym={}, keycode={}, state={}, release={}",
            keysym, keycode, state, is_release
        );

        match proxy.process_key_event(keysym, keycode, state, is_release, time) {
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

    /// Receives and processes pending D-Bus messages/signals.
    /// This is a polling approach. An async approach with signal handlers would be better.
    /// Returns Some(FcitxUpdate) if an update relevant to us was processed.
    pub fn receive_update(&mut self) -> Result<Option<FcitxUpdate>, FepError> {
        // Try to process any pending messages on the connection without blocking indefinitely.
        // `try_receive_message_blocking` or `receive_message_with_timeout` could be used.
        // `process_all_pending` is simpler but might block if handlers do work.
        // Let's use a short timeout.
        match self.connection.receive_message_with_timeout(Duration::from_millis(10)) {
             // Process one message if available within the timeout
            Ok(Some(message)) => {
                // Check if it's a signal for our input context
                if let (Some(interface), Some(member), Some(path)) = (message.interface(), message.member(), message.path()) {
                     // Check if the signal is from the path of our IC proxy
                    if self.ic_path.as_ref().map_or(false, |p| p == path) {
                        // Check if the signal is one we care about from the IC interface
                        if interface == FCITX5_IFACE_IC {
                            match member.as_str() {
                                "CommitString" => {
                                    let (commit_str,): (String,) = message.body()?;
                                    println!("Received CommitString signal: {}", commit_str);
                                    return Ok(Some(FcitxUpdate::CommitString(commit_str)));
                                }
                                "UpdateFormattedPreedit" => {
                                    let (segments, cursor_pos): (Vec<FormattedText>, i32) = message.body()?;
                                    println!("Received UpdateFormattedPreedit signal: {:?}, cursor: {}", segments, cursor_pos);
                                    // Convert FormattedText segments back into a simple string for now
                                    let preedit_str = segments.into_iter().map(|s| s.text).collect::<String>();
                                    // TODO: Handle cursor_pos and formatting properly in terminal.rs
                                    return Ok(Some(FcitxUpdate::UpdatePreedit(preedit_str)));
                                }
                                // Handle other signals like DeleteSurroundingText if needed
                                _ => {
                                    // println!("Received other signal for our IC: {}.{}", interface, member);
                                }
                            }
                        }
                    } else {
                         // println!("Received message for different path: {}", path);
                    }
                } else {
                    // println!("Received non-signal message or message without interface/member/path");
                }
                // If we processed a message but it wasn't an update for us, return None
                 Ok(None)
            }
            Ok(None) => {
                 // Timeout expired, no message received
                 Ok(None)
            }
            Err(zbus::Error::BlockingRecvTimeout(_)) => {
                // Explicitly handle timeout error as Ok(None)
                 Ok(None)
            }
            Err(e) => {
                eprintln!("Error receiving D-Bus message: {}", e);
                Err(FepError::FcitxConnection(format!("Failed to receive/process D-Bus message: {}", e)))
            }
        }

    }

    /// Closes the connection to Fcitx5.
    pub fn disconnect(&mut self) {
        println!("Disconnecting from Fcitx5...");
        if let Some(proxy) = self.ic_proxy.as_mut() {
            if let Err(e) = proxy.focus_out() {
                eprintln!("Error sending FocusOut on disconnect: {}", e);
            }
        }
        // Proxies hold references to the connection, so dropping them is usually enough.
        // The connection itself will be closed when FcitxClient is dropped.
        self.ic_proxy = None;
        self.ic_path = None;
        println!("Fcitx5 disconnected (connection will close on drop).");
    }
}

// Note: No need for manual Drop implementation if Connection handles closure on drop.
// Make sure FcitxClient owns the Connection.
