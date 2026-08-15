use crate::*;

mod lifecycle;
mod workers;

pub async fn handle(cmd: AgentCommands) {
    match cmd {
        AgentCommands::Add {
            name,
            image,
            agent_type,
            model,
            provider,
            base_url,
            port,
            prompt,
            mcp,
            projects,
            workspaces,
        } => {
            workers::add_named(
                name, image, agent_type, model, provider, base_url, port, prompt, mcp, projects,
                workspaces,
            );
        }
        AgentCommands::Remove { name } => workers::remove_named(name),
        AgentCommands::Start {
            name: Some(agent_name),
            project,
            branch,
            port,
        } => {
            if project.is_some() || branch.is_some() {
                eprintln!(
                    "Error: pass either a named agent (`smith settings agent start <name>`) or --project/--branch for a workspace agent, not both."
                );
                std::process::exit(1);
            }
            workers::start_named(agent_name, port);
        }
        AgentCommands::Stop {
            name: Some(agent_name),
            all_named: false,
            ..
        } => workers::stop_named(agent_name),
        AgentCommands::Stop {
            all_named: true, ..
        } => workers::stop_all_named(),
        AgentCommands::List => {
            workers::list_named_section();
            println!();
            lifecycle::handle(AgentCommands::List).await;
        }
        other => lifecycle::handle(other).await,
    }
}
