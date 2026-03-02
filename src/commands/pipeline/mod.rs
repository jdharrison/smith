use crate::*;

pub mod stages;

pub async fn handle(cmd: RunCommands) {
    match cmd {
        cmd @ RunCommands::Plan { .. } => stages::plan::handle(cmd).await,
        cmd @ RunCommands::Develop { .. } => stages::develop::handle(cmd).await,
        cmd @ RunCommands::Release { .. } => stages::release::handle(cmd).await,
        cmd @ RunCommands::Review { .. } => stages::review::handle(cmd).await,
    }
}
