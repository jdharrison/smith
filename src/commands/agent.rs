use crate::*;

mod lifecycle;

pub async fn handle(cmd: AgentCommands) {
    lifecycle::handle(cmd).await;
}
