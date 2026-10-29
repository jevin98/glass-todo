#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appearance;
mod config;
mod core;
#[cfg(windows)]
mod docking;
mod mcp;
mod motion;
#[cfg(windows)]
mod preferences;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod status_manager;
#[cfg(windows)]
mod status_picker;
mod statuses;
mod store;
#[cfg(windows)]
mod ui;

fn main() {
    if let Err(e) = run() {
        #[cfg(windows)]
        unsafe {
            use windows::{
                Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
                core::PCWSTR,
            };
            if !std::env::args().any(|a| a == "--headless") {
                let text: Vec<u16> = format!("{e:#}").encode_utf16().chain(Some(0)).collect();
                MessageBoxW(
                    None,
                    PCWSTR(text.as_ptr()),
                    windows::core::w!("玻璃待办"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        eprintln!("{e:#}");
        std::process::exit(1)
    }
}

fn run() -> anyhow::Result<()> {
    use std::{sync::Arc, time::Duration};
    let cfg = config::Config::parse()?;
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--quit") {
        ui::request_exit(cfg.port)?;
        return Ok(());
    }
    #[cfg(windows)]
    if !cfg.headless && ui::activate_existing(cfg.port) {
        return Ok(());
    }
    let core = Arc::new(core::Core::new(&cfg.directory.join("todos.sqlite3"))?);
    let token = cfg.token()?;
    cfg.write_client_examples(&token)?;
    // Bind before showing the window so a port collision is a visible startup error.
    let socket = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, cfg.port))?;
    socket.set_nonblocking(true)?;
    let stop = tokio_util::sync::CancellationToken::new();
    let server = mcp::spawn(core.clone(), socket, token, stop.clone(), cfg.clone())?;
    if cfg.headless {
        if let Some(seconds) = cfg.exit_after {
            std::thread::sleep(Duration::from_secs(seconds));
            stop.cancel();
        } else {
            server
                .join()
                .map_err(|_| anyhow::anyhow!("MCP 服务线程异常"))?;
            return Ok(());
        }
    } else {
        #[cfg(windows)]
        ui::run(core, cfg.clone())?;
    }
    stop.cancel();
    let _ = server.join();
    Ok(())
}
