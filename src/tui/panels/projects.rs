use crate::tui::app::App;
use crate::tui::state::ProjectStatus;
use ratatui::{
    backend::Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame as TuiFrame,
};

pub fn render_projects_panel(frame: &mut TuiFrame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Projects ")
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(block, area);

    let inner = area.inner(2);

    if app.projects.is_empty() {
        let empty = Paragraph::new("No projects configured. Run `smith project add` to add a project.")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(empty, inner);
        return;
    }

    let rows: Vec<Row> = app.projects.iter().enumerate().map(|(i, project)| {
        let status_color = match project.status {
            ProjectStatus::Active => Color::Green,
            ProjectStatus::Inactive => Color::DarkGray,
            ProjectStatus::Error => Color::Red,
        };
        
        let status_str = format!("{}", project.status);
        
        Row::new(vec![
            if i == app.selected_index { "●" } else { " " }.to_string(),
            project.name.clone(),
            project.repo.clone(),
            project.agent.clone().unwrap_or_else(|| "default".to_string()),
            status_str,
        ])
        .style(if i == app.selected_index {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        })
        .height(1)
    }).collect();

    let table = Table::new(rows)
        .widths([
            ratatui::layout::Constraint::Length(3),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(30),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(10),
        ])
        .column_spacing(1)
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(table, inner);
}
