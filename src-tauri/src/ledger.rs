//! The ledger remembers the crate across restarts: which packages have
//! been delivered before, and the dependency hash that lets BOTTLE be
//! skipped when nothing changed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub dep_hash: Option<String>,
    pub stack: String,
    pub manager: String,
    pub framework: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub entries: Vec<Entry>,
}

impl Ledger {
    pub fn load(file: &Path) -> Ledger {
        fs::read_to_string(file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, file: &Path) {
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = fs::write(file, t);
        }
    }

    pub fn get(&self, path: &Path) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }

    pub fn upsert(&mut self, entry: Entry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.push(entry);
    }

    pub fn remove(&mut self, path: &Path) {
        self.entries.retain(|e| e.path != path);
    }
}

pub fn hash_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Some(format!("{:x}", h.finalize()))
}
