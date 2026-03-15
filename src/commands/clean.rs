use anyhow::Result;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::config;

/// Delete the DB file at the given path.
/// Returns `true` if the file was deleted, `false` if it did not exist.
pub fn delete_db_file(db_path: &Path) -> Result<bool> {
    if db_path.exists() {
        std::fs::remove_file(db_path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub async fn run(config_path: PathBuf, yes: bool, json: bool) -> Result<()> {
    let config = config::load_config(&config_path)?;
    let db_path = config::resolve_db_path(&config)?;

    if !yes {
        eprint!("Delete {}? [y/N]: ", db_path.display());
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let deleted = delete_db_file(&db_path)?;

    if json {
        let output = serde_json::json!({
            "project": config.project,
            "db_path": db_path.display().to_string(),
            "deleted": deleted,
        });
        println!("{}", serde_json::to_string(&output)?);
    } else {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║         Squad Station  •  Clean                              ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();
        println!("  Project  : {}", config.project);
        println!("  Database : {}", db_path.display());
        println!();
        if deleted {
            println!("  [DELETED] database removed.");
        } else {
            println!("  [SKIP]    database not found — nothing to delete.");
        }
        println!();
    }

    Ok(())
}
