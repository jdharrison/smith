use crate::tui::app::App;
use crate::tui::state::{PipelineStage, PipelineStatus};
use ratatui::{
    backend::Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame as TuiFrame,
};

pub fn render_pipelines_panel(frame: &mut TuiFrame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Pipelines ")
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(block, area);

    let inner = area.inner(2);

    if app.pipelines.is_empty() {
        let empty = Paragraph::new("No pipelines running. Select a project and press 'r' to run a pipeline.")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(empty, inner);
        return;
    }

    let rows: Vec<Row> = app.pipelines.iter().enumerate().map(|(i, pipeline)| {
        let status_color = match pipeline.status {
            PipelineStatus::Running => Color::Green,
            PipelineStatus::Pending => Color::Yellow,
            PipelineStatus::Completed => Color::Blue,
            PipelineStatus::Failed => Color::Red,
        };
        
        let status_str = format!("{}", pipeline.status);
        let stage_str = format!("{}", pipeline.stage);
        
        Row::new(vec![
            if i == app.selected_index { "●" } else { " " }.to_string(),
            pipeline.id.clone(),
            pipeline.project.clone(),
            stage_str,
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
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(12),
        ])
        .column_spacing(1)
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(table, inner);

    if let Some(selected) = app.pipelines.get(app.selected_index) {
        if !selected.output.is_empty() {
            let output_area = Rect::new(
                inner.x,
                inner.y + (app.selected_index as u16 + 1) + 1,
                inner.width,
                inner.height.saturating_sub(app.selected_index as u16 + 3),
            );
            
            let output_block = Block::default()
                .borders(Borders::ALL)
                .title(" Output ")
                .style(Style::default().fg(Color::White));
            
            frame.render_widget(output_block, output_area);
            
            let output_text: String = selected.output.iter()
                .rev()
                .take(20)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            
            let output = Paragraph::new(output_text)
                .style(Style::default().fg(Color::DarkGray))
                .scroll((0, 0));
            
            frame.render_widget(output, output_area.inner(2));
        }
    }
}
