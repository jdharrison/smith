use crate::tui::app::App;
use crate::tui::state::{AgentStatus, SystemStatus};
use ratatui::{
    backend::Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame as TuiFrame,
};

pub fn render_dashboard_panel(frame: &mut TuiFrame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Dashboard ")
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(block, area);

    let inner = area.inner(2);

    let running_agents = app.agents.iter().filter(|a| a.status == AgentStatus::Running).count();
    let total_agents = app.agents.len();
    let active_projects = app.projects.len();

    let status_text = format!(
        "System Status\n\
         ────────────\n\
         Docker:      {}\n\
         Smith:      {}\n\
         \n\
         Overview\n\
         ────────\n\
         Agents:     {}/{} running\n\
         Projects:   {} active\n\
         Pipelines:  {} running",
        if app.system_status.docker_available { "✓ Available" } else { "✗ Not found" },
        if app.system_status.smith_installed { "✓ Installed" } else { "✗ Not found" },
        running_agents,
        total_agents,
        active_projects,
        app.pipelines.iter().filter(|p| p.status == crate::tui::state::PipelineStatus::Running).count()
    );

    let paragraph = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(paragraph, inner);
}
