use kgx_core::Result;
use kgx_graph::Brain;

pub trait BrainStore {
    fn open_brain(&self, label: &str) -> Result<Brain>;
    fn home_path(&self) -> std::path::PathBuf;
}

pub struct SqliteBrainStore {
    home_dir: std::path::PathBuf,
}

impl SqliteBrainStore {
    pub fn new() -> Self {
        let home = dirs_data_home();
        Self { home_dir: home }
    }

    pub fn home_brain(&self) -> Result<Brain> {
        let path = self.home_dir.join("brain.sqlite");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Brain::open(&path)
    }

    pub fn project_brain(&self, project: &str) -> Result<Brain> {
        let path = project_brain_path(project);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Brain::open(&path)
    }
}

impl BrainStore for SqliteBrainStore {
    fn open_brain(&self, label: &str) -> Result<Brain> {
        if label == "home" || label == "~" {
            self.home_brain()
        } else {
            self.project_brain(label)
        }
    }

    fn home_path(&self) -> std::path::PathBuf {
        self.home_dir.clone()
    }
}

pub struct BrainSet {
    store: SqliteBrainStore,
    active_projects: Vec<String>,
}

impl BrainSet {
    pub fn new() -> Self {
        let store = SqliteBrainStore::new();
        Self {
            store,
            active_projects: vec![],
        }
    }

    pub fn with_project(mut self, project: &str) -> Self {
        self.active_projects.push(project.to_string());
        self
    }

    pub fn home_brain(&self) -> Result<Brain> {
        self.store.home_brain()
    }

    pub fn project_brains(&self) -> Result<Vec<(String, Brain)>> {
        self.active_projects
            .iter()
            .map(|p| Ok((p.clone(), self.store.project_brain(p)?)))
            .collect()
    }

    /// Query all active projects + home, return union of results
    pub fn query_all<F, T>(&self, f: F) -> Result<Vec<T>>
    where
        F: Fn(&Brain) -> Result<Vec<T>>,
    {
        let mut results = Vec::new();
        if let Ok(home) = self.home_brain() {
            if let Ok(mut r) = f(&home) {
                results.append(&mut r);
            }
        }
        for (_, brain) in self.project_brains()? {
            if let Ok(mut r) = f(&brain) {
                results.append(&mut r);
            }
        }
        Ok(results)
    }
}

fn dirs_data_home() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("KGX_HOME") {
        return std::path::PathBuf::from(home);
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".kgx");
    }
    std::path::PathBuf::from(".kgx")
}

fn project_brain_path(project: &str) -> std::path::PathBuf {
    let base = dirs_data_home();
    base.join("projects")
        .join(project)
        .join(".kg")
        .join("brain.sqlite")
}
