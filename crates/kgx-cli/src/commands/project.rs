use kgx_store::{BrainStore, SqliteBrainStore};

pub fn add(name: &str) -> anyhow::Result<()> {
    let store = SqliteBrainStore::new();
    let brain = store.project_brain(name)?;
    // Verify the brain is accessible
    let cnt: i64 = brain
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))?;
    println!("\u{2714} Project '{name}' ready ({} notes)", cnt);
    // Record in home brain's project registry
    if let Ok(home) = store.home_brain() {
        home.conn()
            .execute(
                "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
                rusqlite::params![format!("project:{name}"), name],
            )
            .ok();
    }
    Ok(())
}

pub fn list() -> anyhow::Result<()> {
    let store = SqliteBrainStore::new();
    if let Ok(home) = store.home_brain() {
        let mut stmt = home
            .conn()
            .prepare("SELECT key, value FROM meta WHERE key LIKE 'project:%'")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (key, name) = row?;
            let proj = key.strip_prefix("project:").unwrap_or(&name);
            println!("  {proj}");
        }
    }
    println!("Home brain: {:?}", store.home_path());
    Ok(())
}

pub fn use_project(name: &str) -> anyhow::Result<()> {
    let store = SqliteBrainStore::new();
    store.project_brain(name)?;
    std::fs::write(store.home_path().join("active_project"), name)?;
    println!("\u{2714} Active project set to '{name}'");
    Ok(())
}

pub fn remove(name: &str) -> anyhow::Result<()> {
    let store = SqliteBrainStore::new();
    if let Ok(home) = store.home_brain() {
        home.conn().execute(
            "DELETE FROM meta WHERE key = ?1",
            rusqlite::params![format!("project:{name}")],
        )?;
    }
    let path = store.home_path().join("projects").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }
    println!("\u{2714} Project '{name}' removed");
    Ok(())
}
