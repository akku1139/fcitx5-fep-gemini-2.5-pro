mod error;
mod event_loop;
mod fcitx;
mod state;
mod terminal;

use error::FepError;
use event_loop::run_event_loop; // run_event_loop も async になる

// #[tokio::main] アトリビュートを追加
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Fcitx5 FEP (Async)...");

    // Terminal は同期的に初期化できる
    let mut terminal = terminal::Terminal::new()?;

    // FcitxClient の接続も async になる
    let mut fcitx_client = fcitx::FcitxClient::connect().await?;

    let mut app_state = state::AppState::new();

    // イベントループを実行 (await する)
    // Ctrl+C ハンドリングもここで行うのが一般的
    tokio::select! {
        result = run_event_loop(&mut terminal, &mut fcitx_client, &mut app_state) => {
            if let Err(e) = result {
                eprintln!("Event loop error: {}", e);
                // エラーが発生してもクリーンアップは Drop で行われる
                // 必要であればここで追加のエラー処理
                return Err(e.into()); // Box<dyn Error> に変換
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C received, shutting down gracefully...");
            // クリーンアップは Terminal と FcitxClient の Drop で行われる
        }
    }

    println!("Exiting Fcitx5 FEP.");
    Ok(())
}
