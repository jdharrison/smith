use crate::tui::state::{
    Agent, AgentStatus, Pipeline, PipelineStage, PipelineStatus, Project, ProjectStatus,
    SystemStatus, UserPreferences,
};
use crate::tui::panels::{
    agents::render_agents_panel, dashboard::render_dashboard_panel,
    pipelines::render_pipelines_panel, projects::render_projects_panel,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, Key, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Agents,
    Projects,
    Pipelines,
}

impl Tab {
    pub fn from_str(s: &str) -> Self {
        match s {
            "agents" => Tab::Agents,
            "projects" => Tab::Projects,
            "pipelines" => Tab::Pipelines,
            _ => Tab::Dashboard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Dashboard => "dashboard",
            Tab::Agents => "agents",
            Tab::Projects => "projects",
            Tab::Pipelines => "pipelines",
        }
    }
}

pub struct App {
    pub current_tab: Tab,
    pub agents: Vec<Agent>,
    pub projects: Vec<Project>,
    pub pipelines: Vec<Pipeline>,
    pub system_status: SystemStatus,
    pub preferences: UserPreferences,
    pub show_help: bool,
    pub selected_index: usize,
    pub should_quit: bool,
    pub message: Option<String>,
    pub last_refresh: Instant,
    pub running_pipeline: Option<String>,
}

impl App {
    pub fn new() -> Self {
        let preferences = crate::tui::db::load_user_preferences();
        let current_tab = Tab::from_str(&preferences.last_active_tab);

        Self {
            current_tab,
            agents: Vec::new(),
            projects: Vec::new(),
            pipelines: Vec::new(),
            system_status: SystemStatus::default(),
            preferences,
            show_help: false,
            selected_index: 0,
            should_quit: false,
            message: None,
            last_refresh: Instant::now(),
            running_pipeline: None,
        }
    }

    pub fn save_preferences(&self) {
        let _ = crate::tui::db::save_user_preferences(&self.preferences);
    }

    pub fn refresh_data(&mut self) {
        self.system_status = check_system_status();
        self.agents = load_agents();
        self.projects = load_projects();
        self.pipelines = load_pipelines();
        self.last_refresh = Instant::now();
    }

}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn check_system_status() -> SystemStatus {
    let docker_available = std::process::Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let smith_installed = std::process::Command::new("smith")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    SystemStatus {
        docker_available,
        dagger_available: false,
        smith_installed,
        version: "0.3.0".to_string(),
    }
}

fn load_agents() -> Vec<Agent> {
    let config_dir = dirs::config_dir()
        .map(|p| p.join("smith"))
        .unwrap_or_default();

    let config_path = config_dir.join("config.toml");
    
    if !config_path.exists() {
        return vec![Agent {
            name: "opencode".to_string(),
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            provider: Some("anthropic".to_string()),
            status: AgentStatus::Stopped,
            agent_type: "cloud".to_string(),
            port: Some(4096),
            enabled: true,
        }];
    }

    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    
    let mut agents = Vec::new();
    
    if let Ok(config) = content.parse::<toml::Value>() {
        if let Some(agents_table) = config.get("agent").and_then(|a| a.as_table()) {
            for (name, value) in agents_table {
                if let Some(agent_table) = value.as_table() {
                    let model = agent_table.get("model").and_then(|m| m.as_str()).map(String::from);
                    let provider = agent_table.get("provider").and_then(|p| p.as_str()).map(String::from);
                    let agent_type = agent_table.get("type").and_then(|t| t.as_str()).unwrap_or("cloud").to_string();
                    let port = agent_table.get("port").and_then(|p| p.as_integer()).map(|p| p as u16);
                    let enabled = agent_table.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true);
                    
                    let docker_running = std::process::Command::new("docker")
                        .args(["ps", "--filter", &format!("name=smith-{}", name), "--format", "{{.Names}}"])
                        .output()
                        .map(|o| {
                            let output = String::from_utf8_lossy(&o.stdout);
                            !output.trim().is_empty()
                        })
                        .unwrap_or(false);

                    let status = if docker_running {
                        AgentStatus::Running
                    } else {
                        AgentStatus::Stopped
                    };

                    agents.push(Agent {
                        name: name.clone(),
                        model,
                        provider,
                        status,
                        agent_type,
                        port,
                        enabled,
                    });
                }
            }
        }
    }

    if agents.is_empty() {
        agents.push(Agent {
            name: "opencode".to_string(),
            model: Some("anthropic/claude-sonnet-4-5".to_string()),
            provider: Some("anthropic".to_string()),
            status: AgentStatus::Stopped,
            agent_type: "cloud".to_string(),
            port: Some(4096),
            enabled: true,
        });
    }

    agents
}

fn load_projects() -> Vec<Project> {
    let config_dir = dirs::config_dir()
        .map(|p| p.join("smith"))
        .unwrap_or_default();

    let projects_dir = config_dir.join("projects");
    
    if !projects_dir.exists() {
        return Vec::new();
    }

    let mut projects = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(config) = content.parse::<toml::Value>() {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        let repo = config.get("repo")
                            .and_then(|r| r.as_str())
                            .unwrap_or("")
                            .to_string();
                        
                        let agent = config.get("agent")
                            .and_then(|a| a.as_str())
                            .map(String::from);
                        
                        let base_branch = config.get("base_branch")
                            .and_then(|b| b.as_str())
                            .unwrap_or("main")
                            .to_string();

                        projects.push(Project {
                            name,
                            repo,
                            agent,
                            status: ProjectStatus::Active,
                            base_branch,
                        });
                    }
                }
            }
        }
    }

    projects
}

fn load_pipelines() -> Vec<Pipeline> {
    Vec::new()
}

pub fn run_tui() -> io::Result<()> {
    if let Err(e) = crate::tui::db::init_db() {
        eprintln!("Warning: Failed to initialize preferences DB: {}", e);
    }
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.refresh_data();

    terminal.clear()?;

    loop {
        let should_refresh = app.last_refresh.elapsed() > Duration::from_millis(app.preferences.refresh_interval);
        if should_refresh && app.running_pipeline.is_none() {
            app.refresh_data();
        }

        terminal.draw(|f| ui(f, &app))?;

        if crossterm::event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                handle_key_event(&mut app, key);
            }
        }

        if app.should_quit {
            app.save_preferences();
            break;
        }

        if let Some(msg) = &app.message {
            if std::time::Instant::now().elapsed() > Duration::from_secs(3) {
                app.message = None;
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_key_event(app: &mut App, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    match key.code {
        Key::Char('q') | Key::Esc => {
            app.should_quit = true;
        }
        Key::Char('?') => {
            app.show_help = !app.show_help;
        }
        Key::Tab => {
            app.current_tab = match app.current_tab {
                Tab::Dashboard => Tab::Agents,
                Tab::Agents => Tab::Projects,
                Tab::Projects => Tab::Pipelines,
                Tab::Pipelines => Tab::Dashboard,
            };
            app.selected_index = 0;
            app.preferences.last_active_tab = app.current_tab.as_str().to_string();
        }
        Key::Down | Key::Char('j') => {
            let max = match app.current_tab {
                Tab::Dashboard => 0,
                Tab::Agents => app.agents.len().saturating_sub(1),
                Tab::Projects => app.projects.len().saturating_sub(1),
                Tab::Pipelines => app.pipelines.len().saturating_sub(1),
            };
            if app.selected_index < max {
                app.selected_index += 1;
            }
        }
        Key::Up | Key::Char('k') => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        Key::Enter | Key::Char('\n') => {
            handle_enter_key(app);
        }
        Key::Char(' ') => {
            handle_space_key(app);
        }
        Key::Char('r') => {
            handle_r_key(app);
        }
        Key::Char('a') => {
            handle_a_key(app);
        }
        Key::Char('d') => {
            handle_d_key(app);
        }
        _ => {}
    }
}

fn handle_enter_key(app: &mut App) {
    match app.current_tab {
        Tab::Agents => {
            if let Some(agent) = app.agents.get(app.selected_index) {
                app.message = Some(format!("Selected agent: {}", agent.name));
            }
        }
        Tab::Projects => {
            if let Some(project) = app.projects.get(app.selected_index) {
                app.message = Some(format!("Selected project: {}", project.name));
            }
        }
        Tab::Pipelines => {
            if let Some(pipeline) = app.pipelines.get(app.selected_index) {
                app.message = Some(format!("Selected pipeline: {}", pipeline.id));
            }
        }
        Tab::Dashboard => {}
    }
}

fn handle_space_key(app: &mut App) {
    if let Tab::Agents = app.current_tab {
        if let Some(agent) = app.agents.get(app.selected_index) {
            let result = if agent.status == AgentStatus::Running {
                crate::tui::panels::agents::stop_agent(&agent.name)
            } else {
                crate::tui::panels::agents::start_agent(&agent.name)
            };
            
            app.message = match result {
                Ok(_) => Some(format!("Agent {} toggled", agent.name)),
                Err(e) => Some(format!("Error: {}", e)),
            };
            
            app.refresh_data();
        }
    }
}

fn handle_r_key(app: &mut App) {
    if let Tab::Projects = app.current_tab {
        if let Some(project) = app.projects.get(app.selected_index) {
            app.message = Some(format!("Run pipeline for: {} (use CLI: smith run develop --project {} --task '...')", project.name, project.name));
        }
    }
}

fn handle_a_key(app: &mut App) {
    if let Tab::Projects = app.current_tab {
        app.message = Some("Add project: use 'smith project add --name <name> --repo <repo>'".to_string());
    }
}

fn handle_d_key(app: &mut App) {
    match app.current_tab {
        Tab::Agents => {
            if let Some(agent) = app.agents.get(app.selected_index) {
                app.message = Some(format!("Delete agent: use 'smith model remove {}'", agent.name));
            }
        }
        Tab::Projects => {
            if let Some(project) = app.projects.get(app.selected_index) {
                app.message = Some(format!("Delete project: use 'smith project remove {}'", project.name));
            }
        }
        _ => {}
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::DarkGray));
    
    let title_text = if app.show_help {
        " Smith Manager - Help ".to_string()
    } else {
        format!(" Smith Manager v{} ", app.system_status.version)
    };
    
    let title = Paragraph::new(title_text)
        .block(title_block)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(title, chunks[0]);

    if app.show_help {
        render_help_overlay(frame, app, chunks[1]);
    } else {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),
                Constraint::Min(0),
            ])
            .split(chunks[1]);

        let tabs = ["Dashboard", "Agents", "Projects", "Pipelines"];
        let active_tab = match app.current_tab {
            Tab::Dashboard => 0,
            Tab::Agents => 1,
            Tab::Projects => 2,
            Tab::Pipelines => 3,
        };

        let sidebar = render_sidebar(tabs, active_tab, app.selected_index);
        frame.render_widget(sidebar, main_chunks[0]);

        let content_area = main_chunks[1];
        
        match app.current_tab {
            Tab::Dashboard => render_dashboard_panel(frame, app, content_area),
            Tab::Agents => render_agents_panel(frame, app, content_area),
            Tab::Projects => render_projects_panel(frame, app, content_area),
            Tab::Pipelines => render_pipelines_panel(frame, app, content_area),
        }
    }

    let help_text = if app.show_help {
        " [Esc] Close ".to_string()
    } else {
        " ↑↓ Navigate  [Enter] Select  [Tab] Switch  [?] Help  [q] Quit ".to_string()
    };
    
    let footer_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::DarkGray));
    
    let footer = Paragraph::new(help_text)
        .block(footer_block)
        .style(Style::default().fg(Color::White));
    frame.render_widget(footer, chunks[2]);
}

fn render_sidebar(tabs: &[&str], active_tab: usize, _selected: usize) -> ratatui::widgets::List {
    use ratatui::widgets::{List, ListItem};
    
    let items: Vec<ListItem> = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let style = if i == active_tab {
                Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!(" {}", tab)).style(style)
        })
        .collect();

    List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Views "))
        .style(Style::default().fg(Color::White))
}

fn render_help_overlay(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::widgets::{Clear, Table, Row, Cell};
    
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts ")
        .style(Style::default().bg(Color::Black));
    
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let help_rows = [
        Row::new(vec![Cell::from("↑/↓"), Cell::from("Navigate items")]),
        Row::new(vec![Cell::from("Tab"), Cell::from("Switch views")]),
        Row::new(vec![Cell::from("Enter"), Cell::from("Select/Confirm")]),
        Row::new(vec![Cell::from("Space"), Cell::from("Start/Stop agent")]),
        Row::new(vec![Cell::from("r"), Cell::from("Run pipeline")]),
        Row::new(vec![Cell::from("a"), Cell::from("Add project")]),
        Row::new(vec![Cell::from("d"), Cell::from("Delete item")]),
        Row::new(vec![Cell::from("?"), Cell::from("Toggle help")]),
        Row::new(vec![Cell::from("q/Esc"), Cell::from("Quit")]),
    ];

    let help_table = Table::new(help_rows)
        .widths([Constraint::Length(15), Constraint::Length(30)])
        .style(Style::default().fg(Color::White));
    
    let inner = area.inner(2);
    frame.render_widget(help_table, inner);
}
