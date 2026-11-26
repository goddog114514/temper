//! TEMPER server entry point.

use std::net::SocketAddr;

use temper_server::state::{AppState, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_env("TEMPER_LOG")
                .unwrap_or_else(|_| EnvFilter::new("temper_server=info,tower_http=warn")),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let config = Config::from_env();
    let static_dir = config.static_dir.clone();
    let addr = config.addr;
    let db_path = config.db_path.clone();

    if !static_dir.join("index.html").exists() {
        tracing::warn!(
            dir = %static_dir.display(),
            "静态目录中没有 index.html；页面会返回 500"
        );
    }

    let state = match AppState::new(config) {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(error = %err, "启动失败");
            std::process::exit(1);
        }
    };

    let app = temper_server::app(state.clone());
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(error = %err, %addr, "无法监听端口");
            std::process::exit(1);
        }
    };
    let local = listener.local_addr().unwrap_or(addr);

    tracing::info!(
        url = %format!("http://{local}"),
        db = %db_path.display(),
        static_dir = %static_dir.display(),
        native = state.native_ok,
        "调律 TEMPER 已启动"
    );

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!(error = %err, "服务异常退出");
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("正在关闭…");
}

#[allow(dead_code)]
fn _addr_is_displayable(addr: &SocketAddr) -> String {
    addr.to_string()
}
