//! Headless preview of the sshc TUI render path.
//!
//! Uses ratatui's TestBackend to render at fixed terminal sizes and dumps
//! the resulting buffer to stdout as raw text. Useful for verifying layout
//! and column spacing without launching the real TUI.
//!
//! Run:
//!   cargo run --example render_preview --release
//!   cargo run --example render_preview --release -- 100 30
//!
//! Args: [width] [height]   (default 100 30)

use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use sshc::app::App;
use sshc::config::model::Host;
use sshc::probe::{ProbeState, ProbeUpdate};
use sshc::ui;

fn fake_host(
    alias: &str,
    user: Option<&str>,
    hostname: &str,
    port: u16,
    source: &str,
    tags: &[&str],
) -> Host {
    Host {
        alias: alias.to_string(),
        hostname: Some(hostname.to_string()),
        user: user.map(|s| s.to_string()),
        port: Some(port),
        identity_file: None,
        line_start: 1,
        source_file: PathBuf::from(source),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        extra: Vec::new(),
        local_forward: None,
        remote_forward: None,
        dynamic_forward: None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let width: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    let height: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);

    // Resolve the real sshc.conf path so the source marker logic mirrors
    // what a fresh install would show.
    let sshc_conf = sshc::storage::sshc_conf_path()
        .unwrap_or_else(|| PathBuf::from("/home/user/.ssh/config.d/sshc.conf"));
    let main_config = PathBuf::from("/home/user/.ssh/config");

    let hosts = vec![
        // sshc.conf-managed, user set, with tags, will be probed Open
        fake_host(
            "web-1",
            Some("root"),
            "web1.example.com",
            22,
            sshc_conf.to_str().unwrap(),
            &["prod", "api"],
        ),
        // sshc.conf-managed, no user (-> $USER fallback dim), Failed probe
        fake_host(
            "db-1",
            None,
            "db1.internal",
            5432,
            sshc_conf.to_str().unwrap(),
            &["prod"],
        ),
        // external source (~/.ssh/config), user set, InFlight probe
        fake_host(
            "staging",
            Some("deploy"),
            "stg.example.com",
            22,
            main_config.to_str().unwrap(),
            &[],
        ),
        // sshc.conf-managed, no user, Unknown (not yet probed)
        fake_host(
            "local",
            None,
            "127.0.0.1",
            2222,
            sshc_conf.to_str().unwrap(),
            &["dev"],
        ),
        // external source, no user, no tags
        fake_host(
            "legacy",
            None,
            "10.0.0.99",
            22,
            main_config.to_str().unwrap(),
            &[],
        ),
    ];

    let mut app = App::new(hosts);
    app.last_connected = Some("web-1".to_string());

    // Seed probe states so the glyph column is exercised.
    app.apply_probe_updates(vec![
        ProbeUpdate {
            host_idx: 0,
            state: ProbeState::Open,
            generation: 1,
        },
        ProbeUpdate {
            host_idx: 1,
            state: ProbeState::Failed,
            generation: 1,
        },
        ProbeUpdate {
            host_idx: 2,
            state: ProbeState::InFlight,
            generation: 1,
        },
        // host 3 stays Unknown
        ProbeUpdate {
            host_idx: 4,
            state: ProbeState::Open,
            generation: 1,
        },
    ]);

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| ui::render(f, &app)).expect("draw");

    let buffer = terminal.backend().buffer().clone();
    println!("┌{}┐  ({}x{})", "─".repeat(width as usize), width, height);
    for y in 0..buffer.area.height {
        print!("│");
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            print!("{}", cell.symbol());
        }
        println!("│");
    }
    println!("└{}┘", "─".repeat(width as usize));
}
