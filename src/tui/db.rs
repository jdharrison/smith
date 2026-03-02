use crate::tui::state::UserPreferences;
use rusqlite::{Connection, Result};
use std::path::PathBuf;

fn get_db_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .map(|p| p.join("smith"))
        .unwrap_or_else(|| PathBuf::from("."));
    
    std::fs::create_dir_all(&config_dir).ok();
    config_dir.join("preferences.db")
}

pub fn init_db() -> Result<()> {
    let conn = Connection::open(get_db_path())?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS preferences (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;
    
    Ok(())
}

pub fn save_preference(key: &str, value: &str) -> Result<()> {
    let conn = Connection::open(get_db_path())?;
    
    conn.execute(
        "INSERT OR REPLACE INTO preferences (key, value) VALUES (?1, ?2)",
        [key, value],
    )?;
    
    Ok(())
}

pub fn load_preference(key: &str) -> Result<Option<String>> {
    let conn = Connection::open(get_db_path())?;
    
    let mut stmt = conn.prepare("SELECT value FROM preferences WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn load_user_preferences() -> UserPreferences {
    let mut prefs = UserPreferences::default_prefs();
    
    if let Ok(Some(tab)) = load_preference("last_active_tab") {
        prefs.last_active_tab = tab;
    }
    
    if let Ok(Some(interval)) = load_preference("refresh_interval") {
        if let Ok(interval) = interval.parse() {
            prefs.refresh_interval = interval;
        }
    }
    
    if let Ok(Some(theme)) = load_preference("theme") {
        prefs.theme = theme;
    }
    
    prefs
}

pub fn save_user_preferences(prefs: &UserPreferences) -> Result<()> {
    save_preference("last_active_tab", &prefs.last_active_tab)?;
    save_preference("refresh_interval", &prefs.refresh_interval.to_string())?;
    save_preference("theme", &prefs.theme)?;
    Ok(())
}
