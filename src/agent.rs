use std::path::PathBuf;
use std::process::Command;

/// Trait for agents that can execute in containers
pub trait Agent {
    /// Initialize the agent with a workspace
    fn initialize(&self, workspace: &PathBuf) -> Result<(), String>;
    
    /// Ask a question and get a response
    fn ask(&self, workspace: &PathBuf, question: &str) -> Result<String, String>;
}

/// OpenCode agent implementation
pub struct OpenCodeAgent;

impl Agent for OpenCodeAgent {
    fn initialize(&self, workspace: &PathBuf) -> Result<(), String> {
        // OpenCode agent initialization - ensure workspace is ready
        if !workspace.exists() {
            return Err("Workspace does not exist".to_string());
        }
        Ok(())
    }
    
    fn ask(&self, workspace: &PathBuf, question: &str) -> Result<String, String> {
        // Normalize question for comparison (lowercase, trim)
        let lower = question.to_lowercase();
        let normalized = lower.trim();
        
        // Check for language-related questions
        if normalized.contains("language") || normalized.contains("what language") {
            detect_project_language(workspace)
        } else {
            Err(format!("Unknown question: {}. Currently supported: questions about project language", question))
        }
    }
}

/// Detect the programming language of a project by checking for common files
/// Uses a container to check for files in the workspace
fn detect_project_language(workspace: &PathBuf) -> Result<String, String> {
    // Check for common project files to determine language
    let language_checks = vec![
        ("Rust", "Cargo.toml"),
        ("JavaScript/TypeScript", "package.json"),
        ("Python", "requirements.txt"),
        ("Python", "pyproject.toml"),
        ("Go", "go.mod"),
        ("Java", "pom.xml"),
        ("Ruby", "Gemfile"),
        ("PHP", "composer.json"),
        ("Swift", "Package.swift"),
    ];
    
    // Use a container to check for files
    let container_name = format!("smith-detect-{}", std::process::id());
    
    // Build command to check for files sequentially, stopping at first match
    let check_commands: Vec<String> = language_checks
        .iter()
        .map(|(lang, file)| {
            format!("test -f /workspace/{} && echo '{}' && exit 0", file, lang)
        })
        .collect();
    
    let full_cmd = format!(
        "cd /workspace && ({}) || echo 'Unknown'",
        check_commands.join(" || ")
    );
    
    let mut cmd = Command::new("docker");
    cmd.arg("run");
    cmd.arg("--rm");
    cmd.arg("--name").arg(&container_name);
    cmd.arg("-v").arg(format!("{}:/workspace:ro", workspace.display()));
    cmd.arg("-w").arg("/workspace");
    cmd.arg("--entrypoint").arg("/bin/sh");
    cmd.arg("alpine:latest");
    cmd.arg("-c").arg(&full_cmd);
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to execute language detection: {}", e))?;
    
    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if result.is_empty() || result == "Unknown" {
            Ok("Unknown".to_string())
        } else {
            Ok(result)
        }
    } else {
        // Fallback: try checking files directly from host
        detect_language_simple(workspace)
    }
}

/// Simple language detection by checking for files directly from host
fn detect_language_simple(workspace: &PathBuf) -> Result<String, String> {
    let checks = vec![
        ("Rust", "Cargo.toml"),
        ("JavaScript/TypeScript", "package.json"),
        ("Python", "requirements.txt"),
        ("Python", "pyproject.toml"),
        ("Go", "go.mod"),
        ("Java", "pom.xml"),
        ("Ruby", "Gemfile"),
        ("PHP", "composer.json"),
    ];
    
    for (lang, file) in checks {
        if workspace.join(file).exists() {
            return Ok(lang.to_string());
        }
    }
    
    Ok("Unknown".to_string())
}

