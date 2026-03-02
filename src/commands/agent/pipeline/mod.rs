use crate::*;

mod stages;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        cmd @ AgentCommands::Plan { .. } => stages::plan::handle(cmd).await,
        cmd @ AgentCommands::Develop { .. } => stages::develop::handle(cmd).await,
        cmd @ AgentCommands::Release { .. } => stages::release::handle(cmd).await,
        cmd @ AgentCommands::Review { .. } => stages::review::handle(cmd).await,
        _ => unreachable!("non-pipeline command routed to pipeline handler"),
    }
}
