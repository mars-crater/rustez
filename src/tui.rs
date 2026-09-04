//! Interactive onboard TUI: welcome → provider → method → model → live dance → done.
//!
//! TTY-only; the flag-driven CLI path in `main.rs` stays for scripts and fallback.

use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};

use super::{run_flow, FlowParams, FlowSummary};
use rustez_agent::bootstrap::DEFAULT_OPENAI_MODEL;
use rustez_agent::oauth;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Welcome,
    Provider,
    Method,
    Model,
    PasteCode,
    Confirm,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Method {
    Browser,
    Device,
    Paste,
}

impl Method {
    fn all() -> [Method; 3] {
        [Method::Browser, Method::Device, Method::Paste]
    }

    fn label(self) -> &'static str {
        match self {
            Method::Browser => "Browser (recommended)",
            Method::Device => "Device code (headless)",
            Method::Paste => "Paste redirect URL",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Method::Browser => "opens auth.openai.com, catches localhost:1455",
            Method::Device => "enter a code at the verification page",
            Method::Paste => "paste the callback code or full redirect URL",
        }
    }
}

enum TuiEvent {
    Log(String),
    Finished(Result<FlowSummary, String>),
}

struct DanceCtx {
    verifier: String,
    state: String,
    url: String,
}

struct App {
    step: Step,
    provider: String,
    docs: String,
    skip_test: bool,
    method_idx: usize,
    model_buf: String,
    paste_buf: String,
    dance: Option<DanceCtx>,
    notice: Option<String>,
    log: Vec<String>,
    started: Option<Instant>,
    rx: Option<mpsc::Receiver<TuiEvent>>,
    summary: Option<FlowSummary>,
    error: Option<String>,
    quit: bool,
    tick: u64,
}

impl App {
    fn new(provider: &str, model: Option<&str>, docs: &str, skip_test: bool) -> Self {
        Self {
            step: Step::Welcome,
            provider: provider.to_string(),
            docs: docs.to_string(),
            skip_test,
            method_idx: 0,
            model_buf: model.unwrap_or("").to_string(),
            paste_buf: String::new(),
            dance: None,
            notice: None,
            log: Vec::new(),
            started: None,
            rx: None,
            summary: None,
            error: None,
            quit: false,
            tick: 0,
        }
    }

    fn method(&self) -> Method {
        Method::all()[self.method_idx % 3]
    }

    fn effective_model(&self) -> String {
        let t = self.model_buf.trim();
        if t.is_empty() {
            DEFAULT_OPENAI_MODEL.to_string()
        } else {
            t.to_string()
        }
    }

    fn build_params(&self) -> FlowParams {
        let (device, paste) = match self.method() {
            Method::Browser => (false, false),
            Method::Device => (true, false),
            Method::Paste => (false, true),
        };
        let paste_code = (self.method() == Method::Paste && !self.paste_buf.trim().is_empty())
            .then(|| self.paste_buf.trim().to_string());
        FlowParams {
            provider: self.provider.clone(),
            model: self.effective_model(),
            docs: self.docs.clone(),
            device,
            paste,
            paste_code,
            verifier: self.dance.as_ref().map(|d| d.verifier.clone()),
            no_open: false,
            skip_test: self.skip_test,
            ask_on_ping_fail: false,
            ask: Box::new(|_| false),
        }
    }

    /// Generate the dance context shown on the paste screen (URL + verifier/state).
    fn begin_paste(&mut self) {
        let (verifier, challenge) = oauth::pkce_pair();
        let state = oauth::new_state();
        let uri = oauth::redirect_uri();
        let url = oauth::authorize_url(&uri, &challenge, &state);
        self.dance = Some(DanceCtx {
            verifier,
            state,
            url,
        });
        self.notice = None;
    }

    /// Validate pasted input against the dance state (bare codes pass through).
    fn accept_paste(&mut self) -> bool {
        let input = self.paste_buf.trim();
        if input.is_empty() {
            return false;
        }
        let expected = self.dance.as_ref().map(|d| d.state.clone());
        match oauth::parse_pasted_code(input) {
            Ok((_, Some(s))) if Some(s.as_str()) != expected.as_deref() => {
                self.notice = Some("state mismatch — paste the URL from THIS run.".to_string());
                false
            }
            Ok(_) => {
                self.notice = None;
                true
            }
            Err(e) => {
                self.notice = Some(format!("bad paste: {e:#}"));
                false
            }
        }
    }

    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 200 {
            let overflow = self.log.len() - 200;
            self.log.drain(..overflow);
        }
    }

    fn start_worker(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.started = Some(Instant::now());
        self.log.clear();
        self.step = Step::Running;
        let params = self.build_params();
        std::thread::spawn(move || {
            let log = |m: &str| {
                let _ = tx.send(TuiEvent::Log(m.to_string()));
            };
            match run_flow(&params, &log) {
                Ok(s) => {
                    let _ = tx.send(TuiEvent::Finished(Ok(s)));
                }
                Err(e) => {
                    let _ = tx.send(TuiEvent::Finished(Err(format!("{e:#}"))));
                }
            }
        });
    }

    fn on_tick(&mut self) {
        self.tick += 1;
        let mut pending = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(ev) = rx.try_recv() {
                pending.push(ev);
                if matches!(pending.last(), Some(TuiEvent::Finished(_))) {
                    break;
                }
            }
        }
        let mut done = None;
        for ev in pending {
            match ev {
                TuiEvent::Log(line) => self.push_log(line),
                TuiEvent::Finished(r) => {
                    done = Some(r);
                    break;
                }
            }
        }
        if let Some(r) = done {
            self.rx = None;
            self.started = None;
            match r {
                Ok(s) => {
                    self.summary = Some(s);
                    self.step = Step::Done;
                }
                Err(e) => {
                    self.error = Some(e);
                    self.step = Step::Failed;
                }
            }
        }
    }

    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match self.step {
            Step::Welcome => match code {
                KeyCode::Enter => self.step = Step::Provider,
                KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                _ => {}
            },
            Step::Provider => match code {
                // v1: openai only — others are listed greyed out.
                KeyCode::Enter => self.step = Step::Method,
                KeyCode::Esc => self.step = Step::Welcome,
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Step::Method => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.method_idx = (self.method_idx + 2) % 3;
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                    self.method_idx = (self.method_idx + 1) % 3;
                }
                KeyCode::Enter => self.step = Step::Model,
                KeyCode::Esc => self.step = Step::Provider,
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Step::Model => match code {
                KeyCode::Char(c) => self.model_buf.push(c),
                KeyCode::Backspace => {
                    self.model_buf.pop();
                }
                KeyCode::Enter => {
                    if self.method() == Method::Paste {
                        self.begin_paste();
                        self.step = Step::PasteCode;
                    } else {
                        self.step = Step::Confirm;
                    }
                }
                KeyCode::Esc => self.step = Step::Method,
                _ => {}
            },
            Step::PasteCode => match code {
                KeyCode::Char(c) => {
                    self.paste_buf.push(c);
                    self.notice = None;
                }
                KeyCode::Backspace => {
                    self.paste_buf.pop();
                    self.notice = None;
                }
                KeyCode::Enter => {
                    if self.accept_paste() {
                        self.step = Step::Confirm;
                    }
                }
                KeyCode::Esc => self.step = Step::Model,
                _ => {}
            },
            Step::Confirm => match code {
                KeyCode::Enter => self.start_worker(),
                KeyCode::Char('t') => self.skip_test = !self.skip_test,
                KeyCode::Esc => self.step = Step::Model,
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Step::Running => {}
            Step::Done | Step::Failed => match code {
                KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                _ => {}
            },
        }
    }
}

/// Run the onboard TUI. Returns the summary on success, `None` on early quit.
pub fn run_tui(
    provider: &str,
    model: Option<&str>,
    docs: &str,
    skip_test: bool,
) -> anyhow::Result<Option<FlowSummary>> {
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.hide_cursor()?;

    let mut app = App::new(provider, model, docs, skip_test);
    loop {
        terminal.draw(|f| draw(f, &app))?;
        if app.quit {
            break;
        }
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key.code, key.modifiers);
            }
        }
        app.on_tick();
    }

    let _ = terminal.show_cursor();
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    Ok(if app.step == Step::Done {
        app.summary
    } else {
        None
    })
}

fn draw(f: &mut Frame, app: &App) {
    let shell = Block::default()
        .title(" RustEZ onboard — ChatGPT OAuth dance ")
        .borders(Borders::ALL);
    let inner = shell.inner(f.area());
    f.render_widget(shell, f.area());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(inner);
    match app.step {
        Step::Welcome => draw_welcome(f, rows[0]),
        Step::Provider => draw_provider(f, rows[0]),
        Step::Method => draw_method(f, rows[0], app),
        Step::Model => draw_input(
            f,
            rows[0],
            "Model",
            "OpenAI model id",
            &app.model_buf,
            DEFAULT_OPENAI_MODEL,
        ),
        Step::PasteCode => draw_paste(f, rows[0], app),
        Step::Confirm => draw_confirm(f, rows[0], app),
        Step::Running => draw_running(f, rows[0], app),
        Step::Done => draw_done(f, rows[0], app),
        Step::Failed => draw_failed(f, rows[0], app),
    }
    f.render_widget(
        Paragraph::new(hints(app)).style(Style::default().fg(Color::DarkGray)),
        rows[1],
    );
}

fn hints(app: &App) -> String {
    match app.step {
        Step::Welcome => "Enter continue · q quit".to_string(),
        Step::Provider => "Enter continue · Esc back".to_string(),
        Step::Method => "↑↓/jk select · Enter continue · Esc back".to_string(),
        Step::Model | Step::PasteCode => "type · Enter continue · Esc back".to_string(),
        Step::Confirm => "Enter run · t toggle ping test · Esc back".to_string(),
        Step::Running => "working… Ctrl-C aborts the UI (dance keeps running)".to_string(),
        Step::Done | Step::Failed => "Enter/q quit".to_string(),
    }
}

fn draw_welcome(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Lightweight agent gateway — first run."),
        Line::from(""),
        Line::from("This wizard signs you in with your ChatGPT account"),
        Line::from("(browser OAuth dance — nothing to paste), picks a model,"),
        Line::from("then points a setup agent at docs/SETUP.md for the rest."),
    ];
    f.render_widget(
        Paragraph::new(text).block(Block::default().title("Welcome")),
        area,
    );
}

fn draw_provider(f: &mut Frame, area: Rect) {
    let items = vec![
        ListItem::new("● openai — ChatGPT OAuth dance"),
        ListItem::new("○ opencode-go — token wizard (TODO)")
            .style(Style::default().fg(Color::DarkGray)),
        ListItem::new("○ chutes — token wizard (TODO)").style(Style::default().fg(Color::DarkGray)),
    ];
    f.render_widget(
        List::new(items)
            .block(Block::default().title("Provider (v1: openai)"))
            .highlight_style(Style::default().fg(Color::Cyan)),
        area,
    );
}

fn draw_method(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = Method::all()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let marker = if i == app.method_idx { "●" } else { "○" };
            ListItem::new(format!("{marker} {} — {}", m.label(), m.hint()))
        })
        .collect();
    f.render_widget(
        List::new(items).block(Block::default().title("Sign-in method")),
        area,
    );
}

fn draw_input(f: &mut Frame, area: Rect, title: &str, label: &str, buf: &str, placeholder: &str) {
    let shown = if buf.is_empty() {
        Line::from(vec![Span::styled(
            placeholder,
            Style::default().fg(Color::DarkGray),
        )])
    } else {
        Line::from(vec![
            Span::raw(buf),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    };
    let text = vec![Line::from(label), Line::from(""), shown];
    f.render_widget(
        Paragraph::new(text).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn draw_paste(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from("Open this URL, approve, then paste the code or full redirect URL:"),
        Line::from(""),
    ];
    match &app.dance {
        Some(d) => lines.push(Line::from(d.url.clone())),
        None => lines.push(Line::from("generating…")),
    }
    lines.push(Line::from(""));
    let shown = if app.paste_buf.is_empty() {
        Line::from(vec![Span::styled(
            "code=… or http://localhost:1455/auth/callback?…",
            Style::default().fg(Color::DarkGray),
        )])
    } else {
        Line::from(vec![
            Span::raw(app.paste_buf.clone()),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    };
    lines.push(shown);
    if let Some(n) = &app.notice {
        lines.push(Line::from(vec![Span::styled(
            n.clone(),
            Style::default().fg(Color::Red),
        )]));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("Paste").borders(Borders::ALL)),
        area,
    );
}

fn draw_confirm(f: &mut Frame, area: Rect, app: &App) {
    let method = app.method();
    let mut lines = vec![
        Line::from(format!("provider  {}", app.provider)),
        Line::from(format!("method    {} ({})", method.label(), method.hint())),
        Line::from(format!("model     {}", app.effective_model())),
        Line::from(format!("docs      {}", app.docs)),
        Line::from(format!(
            "ping test {}  (t toggles)",
            if app.skip_test { "off" } else { "on" }
        )),
    ];
    if method == Method::Paste {
        let short: String = app.paste_buf.trim().chars().take(48).collect();
        lines.push(Line::from(format!("code      {short}")));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter runs the dance. Approve in the browser,"));
    lines.push(Line::from(
        "then the wizard writes rustez.json + docs/SETUP.md.",
    ));
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("Confirm").borders(Borders::ALL)),
        area,
    );
}

fn draw_running(f: &mut Frame, area: Rect, app: &App) {
    const SPIN: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let elapsed = app.started.map(|s| s.elapsed().as_secs()).unwrap_or(0);
    let spin = SPIN[(app.tick as usize) % SPIN.len()];
    let mut lines = vec![Line::from(format!(
        "{spin} signing in… {elapsed}s — approve in the browser"
    ))];
    let tail: Vec<Line> = app
        .log
        .iter()
        .rev()
        .take(12)
        .rev()
        .map(|l| Line::from(l.clone()))
        .collect();
    lines.extend(tail);
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("Working").borders(Borders::ALL)),
        area,
    );
}

fn draw_done(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(vec![Span::styled(
        "Signed in.",
        Style::default().fg(Color::Green),
    )])];
    if let Some(s) = &app.summary {
        lines.push(Line::from(format!("account   {}", s.account_id)));
        if !s.email.is_empty() {
            lines.push(Line::from(format!("email     {}", s.email)));
        }
        lines.push(Line::from(format!("config    {}", s.config_path)));
        lines.push(Line::from(format!("handoff   {}", s.docs_path)));
        if let Some(r) = &s.ping_reply {
            lines.push(Line::from(format!("ping      {r}")));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("The setup agent resumes from the handoff doc:"));
    lines.push(Line::from(
        "Discord → usage → Qdrant → Proton Pass → email.",
    ));
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("Done").borders(Borders::ALL)),
        area,
    );
}

fn draw_failed(f: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(vec![Span::styled(
        "Sign-in failed.",
        Style::default().fg(Color::Red),
    )])];
    if let Some(e) = &app.error {
        lines.push(Line::from(e.clone()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Approved but localhost:1455 unreachable? copy the",
    ));
    lines.push(Line::from(
        "whole address-bar URL and rerun: --paste-code '<url>'",
    ));
    lines.push(Line::from("(saved verifier — no need to approve again)."));
    lines.push(Line::from("Headless? rerun with the device method."));
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("Failed").borders(Borders::ALL)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new("openai", None, "docs/SETUP.md", false)
    }

    #[test]
    fn walk_to_confirm_browser() {
        let mut a = app();
        assert_eq!(a.step, Step::Welcome);
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Provider);
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Method);
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Model);
        for c in "gpt-x".chars() {
            a.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        a.on_key(KeyCode::Backspace, KeyModifiers::NONE);
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Confirm);
        let p = a.build_params();
        assert_eq!(p.model, "gpt-");
        assert!(!p.device && !p.paste && !p.skip_test);
    }

    #[test]
    fn empty_model_falls_back_to_default() {
        let mut a = app();
        a.step = Step::Model;
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.build_params().model, DEFAULT_OPENAI_MODEL);
    }

    #[test]
    fn method_cycles_and_paste_validates_state() {
        let mut a = app();
        a.step = Step::Method;
        a.on_key(KeyCode::Down, KeyModifiers::NONE);
        a.on_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(a.method(), Method::Paste);
        a.on_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(a.method(), Method::Device);
        a.on_key(KeyCode::Down, KeyModifiers::NONE);
        a.step = Step::Model;
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::PasteCode);
        assert!(a.dance.is_some());
        // Empty paste buffer: Enter stays.
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::PasteCode);
        // Wrong state: rejected with notice.
        for c in "http://localhost:1455/auth/callback?code=C&state=WRONG".chars() {
            a.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::PasteCode);
        assert!(a.notice.is_some());
        // Right state: accepted, params carry code + verifier.
        a.paste_buf.clear();
        let state = a.dance.as_ref().expect("dance").state.clone();
        for c in format!("http://localhost:1455/auth/callback?code=C&state={state}").chars() {
            a.on_key(KeyCode::Char(c), KeyModifiers::NONE);
        }
        a.on_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Confirm);
        let p = a.build_params();
        assert!(p.paste);
        assert!(p.paste_code.is_some());
        assert!(p.verifier.is_some());
    }

    #[test]
    fn confirm_toggles_ping_and_backs_out() {
        let mut a = app();
        a.step = Step::Confirm;
        a.on_key(KeyCode::Char('t'), KeyModifiers::NONE);
        assert!(a.build_params().skip_test);
        a.on_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(a.step, Step::Model);
    }

    #[test]
    fn finished_event_drives_done_and_failed() {
        let mut a = app();
        let (tx, rx) = mpsc::channel();
        a.rx = Some(rx);
        a.step = Step::Running;
        tx.send(TuiEvent::Log("hi".to_string())).unwrap();
        tx.send(TuiEvent::Finished(Ok(FlowSummary {
            provider: "openai".to_string(),
            model: "m".to_string(),
            email: "e".to_string(),
            account_id: "a".to_string(),
            config_path: "c".to_string(),
            docs_path: "d".to_string(),
            ping_reply: None,
        })))
        .unwrap();
        a.on_tick();
        assert_eq!(a.step, Step::Done);
        assert_eq!(a.log, vec!["hi".to_string()]);

        let (tx2, rx2) = mpsc::channel();
        a.rx = Some(rx2);
        a.step = Step::Running;
        tx2.send(TuiEvent::Finished(Err("boom".to_string())))
            .unwrap();
        a.on_tick();
        assert_eq!(a.step, Step::Failed);
        assert_eq!(a.error.as_deref(), Some("boom"));
    }

    #[test]
    fn log_caps_at_200() {
        let mut a = app();
        for i in 0..250 {
            a.push_log(format!("l{i}"));
        }
        assert_eq!(a.log.len(), 200);
        assert_eq!(a.log[0], "l50");
    }

    #[test]
    fn ctrl_c_quits_from_anywhere() {
        let mut a = app();
        a.step = Step::Running;
        a.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(a.quit);
    }
}
