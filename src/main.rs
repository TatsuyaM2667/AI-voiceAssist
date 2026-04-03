use anyhow::Result;
use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage},
    Ollama,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, canvas::{Canvas, Context, Line}, Clear, ListState},
    Terminal,
    style::{Color, Style, Modifier},
};
use std::io::{self, Cursor};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink};
use std::process::{Command, Child, Stdio};
use serde::{Serialize, Deserialize};
use std::fs;
use chrono::Local;
use rss::Channel;
use notify::{Watcher, RecursiveMode, Config};

mod voicevox;
mod vosk_engine;
mod audio;

const SESSIONS_DIR: &str = "sessions";

#[derive(Clone, Debug, PartialEq)]
enum AppStatus {
    Idle, Listening, Thinking, Speaking, StartingServices,
}

#[derive(Clone, Debug, PartialEq)]
enum AppMode {
    Chat, SessionSelect,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: String,
    content: String,
}

struct App {
    messages: Vec<Message>,
    status: AppStatus,
    mode: AppMode,
    input: String,
    tick_count: u64,
    active_model: String,
    active_speaker_id: u32,
    speaker_name: String,
    current_session_id: String,
    sessions: Vec<String>,
    session_list_state: ListState,
    latest_news: Arc<Mutex<Vec<String>>>,
}

impl App {
    fn new() -> Self {
        let _ = fs::create_dir_all(SESSIONS_DIR);
        let mut app = App {
            messages: vec![],
            status: AppStatus::StartingServices,
            mode: AppMode::Chat,
            input: String::new(),
            tick_count: 0,
            active_model: "qwen2.5:3b".to_string(),
            active_speaker_id: 14,
            speaker_name: "冥鳴ひまり".to_string(),
            current_session_id: "".into(),
            sessions: vec![],
            session_list_state: ListState::default(),
            latest_news: Arc::new(Mutex::new(vec![])),
        };
        app.refresh_session_list();
        app.new_session();
        app
    }

    fn refresh_session_list(&mut self) {
        if let Ok(entries) = fs::read_dir(SESSIONS_DIR) {
            let mut list: Vec<String> = entries.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|s| s.ends_with(".json")).map(|s| s.replace(".json", "")).collect();
            list.sort_by(|a, b| b.cmp(a));
            self.sessions = list;
        }
    }

    fn new_session(&mut self) {
        self.save_current_session();
        self.current_session_id = format!("chat_{}", Local::now().format("%Y%m%d_%H%M%S"));
        self.messages = vec![];
        self.mode = AppMode::Chat;
    }

    fn load_session(&mut self, id: String) {
        self.save_current_session();
        let path = format!("{}/{}.json", SESSIONS_DIR, id);
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(msgs) = serde_json::from_str(&data) {
                self.messages = msgs; self.current_session_id = id; self.mode = AppMode::Chat;
            }
        }
    }

    fn save_current_session(&self) {
        if self.current_session_id.is_empty() { return; }
        if let Ok(json) = serde_json::to_string(&self.messages) {
            let path = format!("{}/{}.json", SESSIONS_DIR, self.current_session_id);
            let _ = fs::write(path, json);
        }
    }
}

struct ManagedChild(Child);
impl Drop for ManagedChild { fn drop(&mut self) { let _ = self.0.kill(); } }

async fn run_ai_interaction(
    text: String,
    ollama: Ollama,
    model: String,
    speaker_id: u32,
    voicevox: Arc<voicevox::VoicevoxClient>,
    status_tx: mpsc::Sender<AppStatus>,
    msg_tx: mpsc::Sender<Message>,
    latest_news: Arc<Mutex<Vec<String>>>,
) {
    if text.is_empty() { return; }
    msg_tx.send(Message { role: "User".into(), content: text.clone() }).await.ok();
    status_tx.send(AppStatus::Thinking).await.ok();
    
    if text.contains("音楽") || text.contains("ミュージック") {
        msg_tx.send(Message { role: "System".into(), content: "音楽プレイヤーを起動します...".into() }).await.ok();
        let _ = Command::new("sh").arg("-c").arg("cd ~/Documents/music-tui-caelestia && cargo run --release").spawn();
        speak_text("はい、音楽プレイヤーを起動しますね。".to_string(), speaker_id, voicevox.clone(), status_tx.clone(), msg_tx.clone()).await;
        status_tx.send(AppStatus::Idle).await.ok();
        return;
    }

    let mut system_prompt = "あなたは優秀なアシスタントです。必ず日本語で、短く簡潔に回答してください。".to_string();
    if text.contains("ニュース") || text.contains("出来事") {
        let news_list = latest_news.lock().unwrap();
        if !news_list.is_empty() {
            system_prompt.push_str("\n最新ニュース:\n");
            for (i, news) in news_list.iter().take(5).enumerate() { system_prompt.push_str(&format!("{}. {}\n", i+1, news)); }
        }
    }

    let response = ollama.send_chat_messages(ChatMessageRequest::new(model, vec![ChatMessage::system(system_prompt), ChatMessage::user(text)])).await;
    if let Ok(res) = response {
        speak_text(res.message.content, speaker_id, voicevox, status_tx.clone(), msg_tx.clone()).await;
    }
    status_tx.send(AppStatus::Idle).await.ok();
}

async fn speak_text(text: String, speaker_id: u32, voicevox: Arc<voicevox::VoicevoxClient>, status_tx: mpsc::Sender<AppStatus>, msg_tx: mpsc::Sender<Message>) {
    msg_tx.send(Message { role: "Siri".into(), content: text.clone() }).await.ok();
    status_tx.send(AppStatus::Speaking).await.ok();
    if let Ok(audio_data) = voicevox.tts(&text, speaker_id).await {
        let _ = std::thread::spawn(move || {
            if let Ok((_stream, handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&handle) {
                    if let Ok(source) = Decoder::new(Cursor::new(audio_data)) { sink.append(source); sink.sleep_until_end(); }
                }
            }
        }).join();
    }
}

fn draw_visualizer(ctx: &mut Context, status: &AppStatus, tick: u64) {
    let num_spikes = 64; let radius = 18.0;
    let (rotation_speed, amplitude) = match status {
        AppStatus::Thinking => (0.15, 6.0), AppStatus::Speaking => (0.03, 18.0),
        AppStatus::Listening => (0.0, 10.0), _ => (0.02, 3.0),
    };
    let base_angle = (tick as f64) * rotation_speed;
    for i in 0..num_spikes {
        let angle = base_angle + (i as f64) * (2.0 * std::f64::consts::PI / num_spikes as f64);
        let wave = match status {
            AppStatus::Speaking => ((tick as f64 * 0.3 + i as f64 * 0.4).sin() * amplitude).abs(),
            AppStatus::Thinking => (tick as f64 * 0.6).sin() * 4.0 + 4.0,
            _ => ((tick as f64 * 0.1 + i as f64 * 0.2).sin() * amplitude).abs() + 1.0,
        };
        let x1 = angle.cos() * radius; let y1 = angle.sin() * radius;
        let x2 = angle.cos() * (radius + wave); let y2 = angle.sin() * (radius + wave);
        let color = match status {
            AppStatus::Thinking => Color::Cyan, AppStatus::Speaking => Color::Magenta,
            AppStatus::Listening => Color::Red, _ => Color::Blue,
        };
        ctx.draw(&Line { x1, y1, x2, y2, color });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ollama_process = ManagedChild(Command::new("ollama").arg("serve").stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("Ollama error"));
    let _vv_process = ManagedChild(Command::new("/home/tatsuya/.voicevox_extracted/vv-engine/run").arg("--host").arg("127.0.0.1").stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("VOICEVOX error"));

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_struct = App::new();
    let latest_news = Arc::clone(&app_struct.latest_news);
    let app = Arc::new(Mutex::new(app_struct));
    let (status_tx, mut status_rx) = mpsc::channel::<AppStatus>(10);
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(10);
    let ollama = Ollama::default();
    let voicevox_client = Arc::new(voicevox::VoicevoxClient::new("http://localhost:50021"));

    let app_clone = Arc::clone(&app);
    tokio::spawn(async move {
        loop {
             tokio::select! {
                status = status_rx.recv() => { if let Some(status) = status { app_clone.lock().unwrap().status = status; } else { break; } }
                msg = msg_rx.recv() => {
                    if let Some(msg) = msg {
                        let mut app = app_clone.lock().unwrap();
                        app.messages.push(msg); app.save_current_session();
                    } else { break; }
                }
            }
        }
    });

    let s_tx = status_tx.clone(); let m_tx = msg_tx.clone(); let vox = Arc::clone(&voicevox_client);
    let news_ref = Arc::clone(&latest_news);
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let feeds = vec!["https://www3.nhk.or.jp/rss/news/cat0.xml", "https://news.yahoo.co.jp/rss/topics/top-picks.xml"];
        let mut last_alert = String::new();
        loop {
            let mut current = vec![];
            for url in &feeds {
                if let Ok(res) = client.get(*url).send().await {
                    if let Ok(bytes) = res.bytes().await {
                        if let Ok(channel) = Channel::read_from(&bytes[..]) {
                            for item in channel.items() { if let Some(t) = item.title() { current.push(t.to_string()); } }
                        }
                    }
                }
            }
            if !current.is_empty() {
                if &current[0] != &last_alert {
                    if !last_alert.is_empty() { speak_text(format!("ニュース速報です。{}", current[0]), 14, Arc::clone(&vox), s_tx.clone(), m_tx.clone()).await; }
                    last_alert = current[0].clone();
                }
                *news_ref.lock().unwrap() = current;
            }
            tokio::time::sleep(Duration::from_secs(120)).await;
        }
    });

    let s_tx_mail = status_tx.clone(); let m_tx_mail = msg_tx.clone(); let vox_mail = Arc::clone(&voicevox_client);
    tokio::spawn(async move {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::RecommendedWatcher::new(tx, Config::default()).unwrap();
        let mail_path = format!("{}/.thunderbird", std::env::var("HOME").unwrap());
        if let Ok(_) = watcher.watch(std::path::Path::new(&mail_path), RecursiveMode::Recursive) {
            for res in rx {
                if let Ok(_) = res {
                    speak_text("新着メールを受信しました。".to_string(), 14, Arc::clone(&vox_mail), s_tx_mail.clone(), m_tx_mail.clone()).await;
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    });

    let msg_tx_init = msg_tx.clone(); let status_tx_init = status_tx.clone();
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        for _ in 0..30 {
            if client.get("http://localhost:11434/api/tags").send().await.is_ok() && client.get("http://localhost:50021/speakers").send().await.is_ok() {
                msg_tx_init.send(Message { role: "System".into(), content: "Ready!".into() }).await.ok();
                status_tx_init.send(AppStatus::Idle).await.ok(); return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage(35), Constraint::Min(5), Constraint::Length(3), Constraint::Length(3)]).split(f.area());
            
            let mut app_state = app.lock().unwrap();
            app_state.tick_count += 1;
            let status = app_state.status.clone();
            let tick = app_state.tick_count;

            let visualizer = Canvas::default().x_bounds([-50.0, 50.0]).y_bounds([-50.0, 50.0]).paint(move |ctx| { draw_visualizer(ctx, &status, tick); });
            f.render_widget(visualizer, chunks[0]);
            
            let messages: Vec<ListItem> = app_state.messages.iter().rev().take(10).rev().map(|m| {
                let color = match m.role.as_str() { "User" => Color::Yellow, "Siri" => Color::Green, _ => Color::DarkGray };
                ListItem::new(format!("{}: {}", m.role, m.content)).style(Style::default().fg(color))
            }).collect();
            f.render_widget(List::new(messages).block(Block::default().borders(Borders::ALL).title(format!("Chat: {}", app_state.current_session_id))), chunks[1]);
            f.render_widget(Paragraph::new(app_state.input.as_str()).block(Block::default().borders(Borders::ALL).title("Input")), chunks[2]);
            f.render_widget(Paragraph::new(format!("Model: {} | Voice: {} | [N] New [L] List", app_state.active_model, app_state.speaker_name)).block(Block::default().borders(Borders::ALL).title("System Info")), chunks[3]);
            
            if app_state.mode == AppMode::SessionSelect {
                let area = centered_rect(60, 60, f.area()); f.render_widget(Clear, area);
                let session_items: Vec<ListItem> = app_state.sessions.iter().map(|s| ListItem::new(s.clone())).collect();
                let list = List::new(session_items).block(Block::default().borders(Borders::ALL).title("Select Session")).highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)).highlight_symbol(">> ");
                f.render_stateful_widget(list, area, &mut app_state.session_list_state);
            }
        })?;

        if crossterm::event::poll(Duration::from_millis(50))? {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                use crossterm::event::KeyCode;
                let mut app_lock = app.lock().unwrap();
                if app_lock.mode == AppMode::SessionSelect {
                    match key.code {
                        KeyCode::Char('l') | KeyCode::Esc => app_lock.mode = AppMode::Chat,
                        KeyCode::Up => { let i = match app_lock.session_list_state.selected() { Some(i) => if i == 0 { app_lock.sessions.len().saturating_sub(1) } else { i - 1 }, None => 0 }; app_lock.session_list_state.select(Some(i)); }
                        KeyCode::Down => { let i = match app_lock.session_list_state.selected() { Some(i) => if i >= app_lock.sessions.len().saturating_sub(1) { 0 } else { i + 1 }, None => 0 }; app_lock.session_list_state.select(Some(i)); }
                        KeyCode::Enter => { if let Some(i) = app_lock.session_list_state.selected() { if let Some(id) = app_lock.sessions.get(i).cloned() { app_lock.load_session(id); } } }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') if app_lock.input.is_empty() => break,
                        KeyCode::Char('n') if app_lock.input.is_empty() => app_lock.new_session(),
                        KeyCode::Char('l') if app_lock.input.is_empty() => { app_lock.refresh_session_list(); app_lock.mode = AppMode::SessionSelect; }
                        KeyCode::Tab => { app_lock.active_model = if app_lock.active_model == "qwen2.5:3b" { "phi3:mini".into() } else { "qwen2.5:3b".into() }; }
                        KeyCode::Char('s') if app_lock.input.is_empty() => {
                            if app_lock.active_speaker_id == 14 { app_lock.active_speaker_id = 3; app_lock.speaker_name = "ずんだもん".into(); }
                            else if app_lock.active_speaker_id == 3 { app_lock.active_speaker_id = 2; app_lock.speaker_name = "四国めたん".into(); }
                            else { app_lock.active_speaker_id = 14; app_lock.speaker_name = "冥鳴ひまり".into(); }
                        }
                        KeyCode::Char(' ') if app_lock.input.is_empty() => {
                            let (s_tx, m_tx, oll, mod_id, sp_id, vox, news) = (status_tx.clone(), msg_tx.clone(), ollama.clone(), app_lock.active_model.clone(), app_lock.active_speaker_id, Arc::clone(&voicevox_client), Arc::clone(&latest_news));
                            tokio::spawn(async move {
                                s_tx.send(AppStatus::Listening).await.ok();
                                let res = std::thread::spawn(move || {
                                    let r = audio::AudioRecorder::new()?; let d = r.record(3)?;
                                    let v = vosk_engine::VoskClient::new("vosk-model-jp")?; v.transcribe(&d, 44100.0)
                                }).join();
                                if let Ok(Ok(t)) = res { run_ai_interaction(t, oll, mod_id, sp_id, vox, s_tx.clone(), m_tx.clone(), news).await; }
                                s_tx.send(AppStatus::Idle).await.ok();
                            });
                        }
                        KeyCode::Char(c) => app_lock.input.push(c),
                        KeyCode::Backspace => { app_lock.input.pop(); }
                        KeyCode::Enter => {
                            let (input, model, sp_id) = (std::mem::take(&mut app_lock.input), app_lock.active_model.clone(), app_lock.active_speaker_id);
                            if !input.is_empty() {
                                let (s_tx, m_tx, oll, vox, news) = (status_tx.clone(), msg_tx.clone(), ollama.clone(), Arc::clone(&voicevox_client), Arc::clone(&latest_news));
                                tokio::spawn(async move { run_ai_interaction(input, oll, model, sp_id, vox, s_tx, m_tx, news).await; });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    app.lock().unwrap().save_current_session();
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?; Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)]).split(r);
    Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)]).split(popup_layout[1])[1]
}
