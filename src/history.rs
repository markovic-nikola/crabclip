use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ClipContent {
    Text(String),
    Image {
        png_base64: String,
        width: u32,
        height: u32,
    },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id: String,
    pub content: ClipContent,
    pub timestamp: DateTime<Utc>,
    pub pinned: bool,
}

impl ClipEntry {
    pub fn new(content: ClipContent) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            timestamp: Utc::now(),
            pinned: false,
        }
    }
}

pub struct History {
    pub entries: VecDeque<ClipEntry>,
    pub max_size: usize,
}

impl History {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_size,
        }
    }

    pub fn push(&mut self, content: ClipContent) {
        // Dedup: remove any existing unpinned entry with identical content
        self.entries.retain(|e| e.pinned || e.content != content);

        self.entries.push_front(ClipEntry::new(content));
        self.trim();
    }

    pub fn load(path: &Path, max_size: usize) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => {
                let entries: VecDeque<ClipEntry> = serde_json::from_str(&data).unwrap_or_default();
                Self { entries, max_size }
            }
            Err(_) => Self::new(max_size),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.entries)?;
        // Atomic write: write to .tmp then rename
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }

    pub fn toggle_pin(&mut self, id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.pinned = !entry.pinned;
        }
    }

    pub fn move_to_top(&mut self, id: &str) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            if let Some(mut entry) = self.entries.remove(pos) {
                entry.timestamp = chrono::Utc::now();
                self.entries.push_front(entry);
            }
        }
    }

    pub fn clear_unpinned(&mut self) {
        self.entries.retain(|e| e.pinned);
    }

    pub fn search(&self, query: &str) -> Vec<&ClipEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| match &e.content {
                ClipContent::Text(text) => text.to_lowercase().contains(&query_lower),
                ClipContent::Image { .. } => false,
            })
            .collect()
    }

    pub fn trim(&mut self) {
        // Only evict unpinned entries from the back
        while self.entries.len() > self.max_size {
            // Find last unpinned entry and remove it
            if let Some(pos) = self.entries.iter().rposition(|e| !e.pinned) {
                self.entries.remove(pos);
            } else {
                break; // All entries are pinned, can't trim
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_dedup() {
        let mut history = History::new(100);
        history.push(ClipContent::Text("hello".to_string()));
        history.push(ClipContent::Text("world".to_string()));
        history.push(ClipContent::Text("hello".to_string())); // duplicate

        assert_eq!(history.entries.len(), 2);
        // Most recent should be first
        assert!(matches!(&history.entries[0].content, ClipContent::Text(t) if t == "hello"));
        assert!(matches!(&history.entries[1].content, ClipContent::Text(t) if t == "world"));
    }

    #[test]
    fn test_trim_respects_pins() {
        let mut history = History::new(2);
        history.push(ClipContent::Text("a".to_string()));
        history.push(ClipContent::Text("b".to_string()));

        // Pin the oldest entry
        let id = history.entries[1].id.clone();
        history.toggle_pin(&id);

        // Push a third — should evict "b" (unpinned) not "a" (pinned is at back)
        // Wait, after push "b" is at front, "a" is at back. We pinned entries[1] which is "a".
        // Push "c" → now we have ["c", "b", "a(pinned)"], need to trim to 2.
        // Should remove last unpinned = "b" at index 1.
        history.push(ClipContent::Text("c".to_string()));

        assert_eq!(history.entries.len(), 2);
        assert!(matches!(&history.entries[0].content, ClipContent::Text(t) if t == "c"));
        assert!(history.entries[1].pinned);
    }

    #[test]
    fn test_save_and_load() {
        let dir = std::env::temp_dir().join("crabclip_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_history.json");

        let mut history = History::new(100);
        history.push(ClipContent::Text("test entry".to_string()));
        history.save(&path).unwrap();

        let loaded = History::load(&path, 100);
        assert_eq!(loaded.entries.len(), 1);
        assert!(matches!(&loaded.entries[0].content, ClipContent::Text(t) if t == "test entry"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search() {
        let mut history = History::new(100);
        history.push(ClipContent::Text("Hello World".to_string()));
        history.push(ClipContent::Text("foo bar".to_string()));
        history.push(ClipContent::Text("hello again".to_string()));

        let results = history.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_remove_and_clear() {
        let mut history = History::new(100);
        history.push(ClipContent::Text("a".to_string()));
        history.push(ClipContent::Text("b".to_string()));

        let id = history.entries[0].id.clone();
        history.toggle_pin(&id);
        history.clear_unpinned();

        assert_eq!(history.entries.len(), 1);
        assert!(history.entries[0].pinned);

        history.remove(&id);
        assert!(history.entries.is_empty());
    }
}
