use crate::tui::app::App;
use crate::tui::state::AgentStatus;
use ratatui::{
    backend::Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame as TuiFrame,
};

pub fn render_agents_panel(frame: &mut TuiFrame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Agents ")
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(block, area);

    let inner = area.inner(2);

    if app.agents.is_empty() {
        let empty = Paragraph::new("No agents configured. Run `smith model add` to add an agent.")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(empty, inner);
        return;
    }

    let rows: Vec<Row> = app.agents.iter().enumerate().map(|(i, agent)| {
        let status_color = match agent.status {
            AgentStatus::Running => Color::Green,
            AgentStatus::Stopped => Color::DarkGray,
            AgentStatus::Unhealthy => Color::Red,
            AgentStatus::Building => Color::Yellow,
        };
        
        let status_str = format!("{}", agent.status);
        
        let row = Row::new(vec![
            if i == app.selected_index { "●" } else { " " }.to_string(),
            agent.name.clone(),
            agent.model.clone().unwrap_or_else(|| "N/A".to_string()),
            agent.provider.clone().unwrap_or_else(|| "N/A".to_string()),
            status_str,
        ])
        .style(if i == app.selected_index {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        })
        .height(1);
        
        row
    }).collect();

    let table = Table::new(rows)
        .widths([
            ratatui::layout::Constraint::Length(3),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(25),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(12),
        ])
        .column_spacing(1)
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(table, inner);
}

pub fn start_agent(agent_name: &str) -> Result<(), String> {
    let output = std::process::Command::new("smith")
        .args(["model", "start"])
        .output()
        .map_err(|e| format!("Failed to start agent: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn stop_agent(agent_name: &str) -> Result<(), String> {
    let output = std::process::Command::new("docker")
        .args(["stop", &format!("smith-{}", agent_name)])
        .output()
        .map_err(|e| format!("Failed to stop agent: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
