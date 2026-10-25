//! Export the real Ratatui renderer with synthetic data, without mining/network I/O.
use super::*;
use ratatui::backend::TestBackend;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#b4bec8".to_string(),
    }
}

fn capture(state: &TuiStateInner, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| draw_dashboard(frame, frame.area(), state))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut html = String::from("<pre>");
    for y in 0..height {
        let mut previous = None;
        for x in 0..width {
            let cell = &buffer[(x, y)];
            if previous != Some(cell.fg) {
                if previous.is_some() {
                    html.push_str("</span>");
                }
                html.push_str(&format!("<span style='color:{}'>", color(cell.fg)));
                previous = Some(cell.fg);
            }
            html.push_str(&escape(cell.symbol()));
        }
        html.push_str("</span>\n");
    }
    html.push_str("</pre>");
    html
}

#[test]
#[ignore = "writes the feature showcase; run explicitly after changing thread feedback"]
fn export_thread_controls_showcase() {
    let mut html = String::from(
        r#"<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1"><title>Seine thread controls</title>
<style>body{margin:16px;background:#101318;color:#b4bec8;font:14px system-ui}h1{font-size:22px}h2{font-size:16px}pre{background:#080b10;padding:8px;font:12px/1.25 monospace;width:max-content;max-width:100%;overflow:auto}section{margin:24px 0}.mobile{display:none}@media(max-width:700px){body{margin:12px}.desktop{display:none}.mobile{display:block}pre{font-size:12px}}</style>
<h1>Seine CPU thread controls</h1><p>Real terminal components, synthetic mining data. No miner or network connection is started.</p>"#,
    );
    let revision = std::process::Command::new("git")
        .args(["hash-object", "src/miner/tui.rs"])
        .output()
        .unwrap();
    html.push_str(&format!(
        "<p>Renderer source blob: <code>{}</code></p>",
        String::from_utf8_lossy(&revision.stdout).trim()
    ));
    for (name, mode) in [
        ("Ready", 0),
        ("Loading / restart requested", 1),
        ("Disabled / waiting for restart", 2),
        ("At the safe thread limit", 3),
        ("At the minimum thread count", 4),
        ("RAM fell before admission", 5),
        ("Control channel unavailable", 6),
        ("Empty / no CPU backend", 7),
        ("Success / replacement session", 8),
    ] {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let mut state = TuiStateInner::new();
        state.mode = "pool".into();
        state.state = "hashing".into();
        state.threads = 2;
        state.pool_worker = "demo-worker".into();
        state.api_url = "stratum://pool.example:3333".into();
        state.backends_desc = "cpu".into();
        state.round_hashrate = "2.500 H/s".into();
        state.avg_hashrate = "2.400 H/s".into();
        state.thread_control = Some(ThreadControl {
            run_id: 1,
            sender,
            max_threads: 4,
            current_threads: 2,
            memory: None,
            quit_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let key = |state: &mut TuiStateInner, ch| {
            state.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        };
        match mode {
            1 => key(&mut state, '+'),
            2 => {
                key(&mut state, '+');
                key(&mut state, '+');
            }
            3 => {
                state.thread_control.as_mut().unwrap().max_threads = 2;
                key(&mut state, '+');
            }
            4 => {
                state.threads = 1;
                state.thread_control.as_mut().unwrap().current_threads = 1;
                key(&mut state, '-');
            }
            5 => {
                key(&mut state, '+');
                let request = receiver.try_recv().unwrap();
                let message = super::super::thread_control::validate_headroom(3, 2)
                    .unwrap_err()
                    .to_string();
                request.rejection.unwrap().send(message).unwrap();
                state.poll_thread_rejection();
            }
            6 => {
                drop(receiver);
                key(&mut state, '+');
            }
            7 => {
                state.threads = 0;
                state.thread_control = None;
                state.backends_desc.clear();
                key(&mut state, '+');
            }
            8 => {
                state.threads = 3;
                state.thread_control.as_mut().unwrap().current_threads = 3;
            }
            _ => {}
        }
        html.push_str(&format!("<section><h2>{name}</h2><div class='desktop'>{}</div><div class='mobile'>{}</div></section>", capture(&state, 120, 30), capture(&state, 40, 18)));
    }
    html.push_str("</html>");
    let path = std::path::Path::new("docs/showcases/thread-controls");
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("index.html"), html).unwrap();
}
