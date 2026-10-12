//! The file explorer under Tools: a look round the whole card, with copy,
//! cut and paste, rename, delete and new folders.
//!
//! Paths are card-relative throughout. `System` and the updater's folder
//! can be looked in and copied from but never changed, nor can the card's
//! own top-level folders be renamed or deleted. Symbolic links are listed
//! but never followed, gone into or written through.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// Folders whose insides slot runs from.
const LOCKED: [&str; 2] = ["System", ".tmp_update"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    /// A symbolic link: shown, never followed.
    pub link: bool,
}

/// What Copy or Cut picked up, card-relative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub items: Vec<PathBuf>,
    pub cut: bool,
}

#[derive(Clone, Debug)]
pub struct Explorer {
    root: PathBuf,
    /// The folder shown, card-relative; empty for the card itself.
    pub cwd: PathBuf,
    pub entries: Vec<FileEntry>,
    pub at: usize,
    /// Names in the folder shown, ticked with Y.
    pub ticked: BTreeSet<String>,
    pub hidden: bool,
    pub clip: Option<Clip>,
}

/// Whether a card-relative path is one slot may change: not the card
/// itself, nor inside `System` or the updater's folder.
pub fn changeable(rel: &Path) -> bool {
    match rel.components().next() {
        None => false,
        Some(Component::Normal(top)) => !LOCKED.iter().any(|l| top == *l),
        Some(_) => false,
    }
}

/// Whether an item may be renamed, moved away or deleted: changeable, and
/// not one of the card's own top-level folders.
pub fn removable(rel: &Path) -> bool {
    changeable(rel) && rel.components().count() > 1
}

/// Whether a name typed for a file or folder is one: not empty, not `.`
/// or `..`, and with no slashes.
pub fn good_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

pub fn list(root: &Path, rel: &Path, hidden: bool) -> Vec<FileEntry> {
    let Ok(dir) = std::fs::read_dir(root.join(rel)) else {
        return Vec::new();
    };
    let mut out: Vec<FileEntry> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                return None;
            }
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            let link = meta.file_type().is_symlink();
            Some(FileEntry {
                name,
                dir: meta.is_dir(),
                size: if meta.is_file() { meta.len() } else { 0 },
                link,
            })
        })
        .collect();
    out.sort_by_key(|e| (!e.dir, e.name.to_lowercase()));
    out
}

/// `name` in `dir`, or `name (2)`, `name (3)`... if that is taken.
pub fn free_name(dir: &Path, name: &str) -> String {
    if std::fs::symlink_metadata(dir.join(name)).is_err() {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|n| std::fs::symlink_metadata(dir.join(n)).is_err())
        .unwrap_or_else(|| name.to_string())
}

impl Explorer {
    pub fn new(root: &Path) -> Explorer {
        let mut e = Explorer {
            root: root.to_path_buf(),
            cwd: PathBuf::new(),
            entries: Vec::new(),
            at: 0,
            ticked: BTreeSet::new(),
            hidden: false,
            clip: None,
        };
        e.refresh();
        e
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reads the folder again, keeping the cursor on the same name.
    pub fn refresh(&mut self) {
        let keep = self.current().map(|e| e.name.clone());
        self.entries = list(&self.root, &self.cwd, self.hidden);
        self.ticked
            .retain(|t| self.entries.iter().any(|e| e.name == *t));
        self.at = keep
            .and_then(|k| self.entries.iter().position(|e| e.name == k))
            .unwrap_or(self.at)
            .min(self.entries.len().saturating_sub(1));
    }

    pub fn current(&self) -> Option<&FileEntry> {
        self.entries.get(self.at)
    }

    pub fn step(&mut self, by: i32) {
        let n = self.entries.len() as i32;
        if n == 0 {
            return;
        }
        self.at = (self.at as i32 + by).clamp(0, n - 1) as usize;
    }

    /// Down a line, wrapping from the end to the top, and up likewise.
    pub fn wrap(&mut self, by: i32) {
        let n = self.entries.len() as i32;
        if n == 0 {
            return;
        }
        self.at = (self.at as i32 + by).rem_euclid(n) as usize;
    }

    /// Goes into the folder under the cursor, saying whether it did.
    pub fn enter(&mut self) -> bool {
        match self.current().cloned() {
            Some(e) if e.dir && !e.link => {
                self.cwd.push(&e.name);
                self.at = 0;
                self.ticked.clear();
                self.refresh();
                true
            }
            _ => false,
        }
    }

    /// Goes up a folder, onto the one it came from; false at the card.
    pub fn up(&mut self) -> bool {
        let Some(name) = self
            .cwd
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            return false;
        };
        self.cwd.pop();
        self.ticked.clear();
        self.entries = list(&self.root, &self.cwd, self.hidden);
        self.at = self
            .entries
            .iter()
            .position(|e| e.name == name)
            .unwrap_or(0);
        true
    }

    pub fn tick(&mut self) {
        if let Some(name) = self.current().map(|e| e.name.clone()) {
            if !self.ticked.remove(&name) {
                self.ticked.insert(name);
            }
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.hidden = !self.hidden;
        self.refresh();
    }

    /// What an action works on: the ticked items, or else the one under
    /// the cursor; card-relative.
    pub fn chosen(&self) -> Vec<PathBuf> {
        match self.ticked.is_empty() {
            false => self.ticked.iter().map(|n| self.cwd.join(n)).collect(),
            true => self
                .current()
                .map(|e| vec![self.cwd.join(&e.name)])
                .unwrap_or_default(),
        }
    }

    /// Whether anything can be pasted or made in the folder shown.
    pub fn here_changeable(&self) -> bool {
        changeable(&self.cwd)
    }

    pub fn copy(&mut self, cut: bool) -> usize {
        let items = self.chosen();
        let n = items.len();
        if n > 0 {
            self.clip = Some(Clip { items, cut });
            self.ticked.clear();
        }
        n
    }

    /// Renames the item under the cursor.
    pub fn rename(&mut self, to: &str) -> Result<(), String> {
        let Some(e) = self.current().cloned() else {
            return Err("Nothing to rename".into());
        };
        let from = self.cwd.join(&e.name);
        if !removable(&from) {
            return Err("This can't be renamed".into());
        }
        let to = to.trim();
        if !good_name(to) {
            return Err("That isn't a name".into());
        }
        let dir = self.root.join(&self.cwd);
        if std::fs::symlink_metadata(dir.join(to)).is_ok() {
            return Err("That name is taken".into());
        }
        std::fs::rename(dir.join(&e.name), dir.join(to)).map_err(|e| e.to_string())?;
        self.refresh();
        if let Some(i) = self.entries.iter().position(|x| x.name == to) {
            self.at = i;
        }
        Ok(())
    }

    pub fn new_folder(&mut self, name: &str) -> Result<(), String> {
        if !changeable(&self.cwd) {
            return Err("Nothing can be added here".into());
        }
        let name = name.trim();
        if !good_name(name) {
            return Err("That isn't a name".into());
        }
        let dir = self.root.join(&self.cwd);
        if std::fs::symlink_metadata(dir.join(name)).is_ok() {
            return Err("That name is taken".into());
        }
        std::fs::create_dir(dir.join(name)).map_err(|e| e.to_string())?;
        self.refresh();
        if let Some(i) = self.entries.iter().position(|x| x.name == name) {
            self.at = i;
        }
        Ok(())
    }
}

/// A slow job: pasting or deleting, done on a thread of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileJob {
    Paste {
        items: Vec<PathBuf>,
        cut: bool,
        into: PathBuf,
    },
    Delete {
        items: Vec<PathBuf>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileNews {
    /// Bytes done of the total, and what is being worked on.
    Progress {
        done: u64,
        total: u64,
        name: String,
    },
    Done {
        items: usize,
        errors: Vec<String>,
    },
}

/// Bytes and files under a path, not following links.
fn weigh(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        std::fs::read_dir(path)
            .map(|d| d.flatten().map(|e| weigh(&e.path())).sum())
            .unwrap_or(0)
    } else {
        meta.len().max(1)
    }
}

struct Work<'a> {
    done: u64,
    total: u64,
    news: &'a Sender<FileNews>,
    stop: &'a AtomicBool,
}

impl Work<'_> {
    fn tell(&self, name: &str) {
        let _ = self.news.send(FileNews::Progress {
            done: self.done,
            total: self.total,
            name: name.to_string(),
        });
    }

    fn copy_tree(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if self.stop.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("stopped"));
        }
        let meta = std::fs::symlink_metadata(from)?;
        if meta.file_type().is_symlink() {
            // Links are left behind: they could lead anywhere.
            return Ok(());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(to)?;
            for e in std::fs::read_dir(from)?.flatten() {
                self.copy_tree(&e.path(), &to.join(e.file_name()))?;
            }
            return Ok(());
        }
        let name = from
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.tell(&name);
        let mut src = std::fs::File::open(from)?;
        let mut dst = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if self.stop.load(Ordering::Relaxed) {
                drop(dst);
                let _ = std::fs::remove_file(to);
                return Err(std::io::Error::other("stopped"));
            }
            let n = std::io::Read::read(&mut src, &mut buf)?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut dst, &buf[..n])?;
            self.done += n as u64;
            self.tell(&name);
        }
        dst.sync_all()?;
        if let Ok(when) = meta.modified() {
            let _ = dst.set_modified(when);
        }
        self.done += u64::from(meta.len() == 0);
        Ok(())
    }
}

fn remove_tree(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Whether `inner` is `outer` or inside it.
fn within(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}

/// Does a job, telling `news` how it goes.
pub fn run(root: &Path, job: &FileJob, news: &Sender<FileNews>, stop: &AtomicBool) {
    let mut errors = Vec::new();
    let mut items = 0;
    match job {
        FileJob::Paste {
            items: from,
            cut,
            into,
        } => {
            let refused = !changeable(into);
            if refused {
                errors.push("Nothing can be pasted here".to_string());
            }
            let total = from.iter().map(|p| weigh(&root.join(p))).sum();
            let mut work = Work {
                done: 0,
                total,
                news,
                stop,
            };
            for rel in from.iter().filter(|_| !refused) {
                let name = rel
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if within(into, rel) {
                    errors.push(format!("{name} can't go inside itself"));
                    continue;
                }
                if *cut && !removable(rel) {
                    errors.push(format!("{name} can't be moved"));
                    continue;
                }
                let src = root.join(rel);
                if std::fs::symlink_metadata(&src).is_err() {
                    errors.push(format!("{name} is gone"));
                    continue;
                }
                let dir = root.join(into);
                let same_place = rel.parent() == Some(into.as_path());
                if *cut && same_place {
                    items += 1;
                    continue;
                }
                let dest = dir.join(free_name(&dir, &name));
                let moved = *cut && std::fs::rename(&src, &dest).is_ok();
                let result = match moved {
                    true => {
                        work.done += weigh(&dest);
                        work.tell(&name);
                        Ok(())
                    }
                    false => work.copy_tree(&src, &dest).and_then(|_| {
                        if *cut {
                            remove_tree(&src)
                        } else {
                            Ok(())
                        }
                    }),
                };
                match result {
                    Ok(()) => items += 1,
                    Err(e) => {
                        errors.push(format!("{name}: {e}"));
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }
        }
        FileJob::Delete { items: gone } => {
            let total = gone.len() as u64;
            for (i, rel) in gone.iter().enumerate() {
                let name = rel
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let _ = news.send(FileNews::Progress {
                    done: i as u64,
                    total,
                    name: name.clone(),
                });
                if !removable(rel) {
                    errors.push(format!("{name} can't be deleted"));
                    continue;
                }
                match remove_tree(&root.join(rel)) {
                    Ok(()) => items += 1,
                    Err(e) => errors.push(format!("{name}: {e}")),
                }
            }
        }
    }
    let _ = news.send(FileNews::Done { items, errors });
}

/// A job on its own thread.
pub struct FileWorker {
    pub news: Receiver<FileNews>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for FileWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FileWorker")
    }
}

impl FileWorker {
    pub fn start(root: &Path, job: FileJob) -> FileWorker {
        let (tell, news) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let root = root.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("slot-files-job".into())
            .spawn(move || run(&root, &job, &tell, &flag));
        if let Err(e) = spawned {
            eprintln!("slot: files: {e}");
        }
        FileWorker { news, stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_and_top_level_folders_are_kept_safe() {
        assert!(!changeable(Path::new("")));
        assert!(!changeable(Path::new("System/slot")));
        assert!(!changeable(Path::new(".tmp_update/updater")));
        assert!(changeable(Path::new("Games")));
        assert!(!removable(Path::new("Games")));
        assert!(removable(Path::new("Games/GBA")));
        assert!(good_name("Pokémon (2).gba"));
        for bad in ["", " ", ".", "..", "a/b", "a\\b"] {
            assert!(!good_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.gba"), b"").unwrap();
        std::fs::write(d.path().join("a (2).gba"), b"").unwrap();
        std::fs::create_dir(d.path().join("Folder")).unwrap();
        assert_eq!(free_name(d.path(), "b.gba"), "b.gba");
        assert_eq!(free_name(d.path(), "a.gba"), "a (3).gba");
        assert_eq!(free_name(d.path(), "Folder"), "Folder (2)");
    }
}
