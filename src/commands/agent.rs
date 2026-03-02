use crate::*;

mod lifecycle;
mod pipeline;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        cmd @ (AgentCommands::Plan { .. }
        | AgentCommands::Develop { .. }
        | AgentCommands::Release { .. }
        | AgentCommands::Review { .. }) => pipeline::handle(cmd).await,
        cmd => lifecycle::handle(cmd).await,
    }
}
