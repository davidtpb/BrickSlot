use slot_store::{fold, rank, Platform};

pub const ROWS: usize = 4;
pub const COLS: usize = 10;

pub const SPACE: &str = "SPC";
pub const DELETE: &str = "DEL";

/// The on-screen keyboard, row by row.
pub const KEYS: [[&str; COLS]; ROWS] = [
    ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"],
    ["K", "L", "M", "N", "O", "P", "Q", "R", "S", "T"],
    ["U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3"],
    ["4", "5", "6", "7", "8", "9", "-", "'", SPACE, DELETE],
];

pub const SHOWN: usize = 4;

const QUERY_MAX: usize = 24;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hit {
    pub shelf: usize,
    pub index: usize,
    pub name: String,
    pub platform: Platform,
}

#[derive(Clone, Debug, Default)]
pub struct Search {
    pub query: String,
    pub row: usize,
    pub col: usize,
    pub hits: Vec<Hit>,
    pub pick: usize,
}

impl Search {
    pub fn key(&self) -> &'static str {
        KEYS[self.row][self.col]
    }

    pub fn step(&mut self, dx: i32, dy: i32) {
        self.col = (self.col as i32 + dx).rem_euclid(COLS as i32) as usize;
        self.row = (self.row as i32 + dy).rem_euclid(ROWS as i32) as usize;
    }

    /// Types the key under the cursor. Returns whether the query changed.
    pub fn type_key(&mut self) -> bool {
        match self.key() {
            DELETE => self.delete(),
            SPACE if self.query.is_empty() || self.query.ends_with(' ') => false,
            SPACE => self.push(' '),
            key => key.chars().next().is_some_and(|c| self.push(c)),
        }
    }

    fn push(&mut self, c: char) -> bool {
        if self.query.chars().count() >= QUERY_MAX {
            return false;
        }
        self.query.push(c);
        true
    }

    /// Drops the last character. Returns false when there was none.
    pub fn delete(&mut self) -> bool {
        self.query.pop().is_some()
    }

    pub fn pick_by(&mut self, by: i32) {
        let n = self.hits.len().min(SHOWN);
        if n == 0 {
            return;
        }
        self.pick = (self.pick as i32 + by).rem_euclid(n as i32) as usize;
    }

    pub fn picked(&self) -> Option<&Hit> {
        self.hits.get(self.pick)
    }

    /// Matches the query against every cart on every shelf, best first:
    /// names that start with it, then a word that does, then the rest, each
    /// in shelf order.
    pub fn refresh<'a>(&mut self, shelves: impl Iterator<Item = (Platform, &'a [String])>) {
        let query = fold(&self.query);
        let mut found: Vec<(u8, Hit)> = Vec::new();
        for (shelf, (platform, stems)) in shelves.enumerate() {
            for (index, stem) in stems.iter().enumerate() {
                if let Some(r) = rank(&query, stem) {
                    found.push((
                        r,
                        Hit {
                            shelf,
                            index,
                            name: stem.clone(),
                            platform,
                        },
                    ));
                }
            }
        }
        found.sort_by_key(|(r, h)| (*r, h.shelf, h.index));
        self.hits = found.into_iter().map(|(_, h)| h).collect();
        self.pick = 0;
    }
}
