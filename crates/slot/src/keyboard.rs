//! The on-screen keyboard for typing a Wi-Fi password: lower case, upper
//! case and symbols, ten keys to a row, on the search screen's grid.

use crate::search::{COLS, ROWS};

pub const LAYERS: [[[&str; COLS]; ROWS]; 3] = [
    [
        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
        ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
        ["a", "s", "d", "f", "g", "h", "j", "k", "l", "-"],
        ["z", "x", "c", "v", "b", "n", "m", "_", ".", "@"],
    ],
    [
        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
        ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"],
        ["A", "S", "D", "F", "G", "H", "J", "K", "L", "-"],
        ["Z", "X", "C", "V", "B", "N", "M", "_", ".", "@"],
    ],
    [
        ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")"],
        ["~", "`", "-", "_", "=", "+", "[", "]", "{", "}"],
        ["\\", "|", ";", ":", "'", "\"", ",", ".", "<", ">"],
        ["/", "?", "€", "£", "¥", "§", "°", "¿", "¡", "·"],
    ],
];

pub const LAYER_NAMES: [&str; 3] = ["abc", "ABC", "#+="];

/// The longest passphrase WPA takes.
pub const PASSWORD_MAX: usize = 63;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keyboard {
    pub text: String,
    pub row: usize,
    pub col: usize,
    pub layer: usize,
}

impl Keyboard {
    pub fn key(&self) -> &'static str {
        LAYERS[self.layer][self.row][self.col]
    }

    pub fn step(&mut self, dx: i32, dy: i32) {
        self.col = (self.col as i32 + dx).rem_euclid(COLS as i32) as usize;
        self.row = (self.row as i32 + dy).rem_euclid(ROWS as i32) as usize;
    }

    pub fn next_layer(&mut self) {
        self.layer = (self.layer + 1) % LAYERS.len();
    }

    pub fn type_key(&mut self) {
        let key = self.key();
        self.push(key);
    }

    pub fn space(&mut self) {
        self.push(" ");
    }

    fn push(&mut self, s: &str) {
        if self.text.chars().count() < PASSWORD_MAX {
            self.text.push_str(s);
        }
    }

    /// Takes off the last character, saying whether there was one.
    pub fn delete(&mut self) -> bool {
        self.text.pop().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_fills_the_grid_with_one_character_keys() {
        for layer in LAYERS {
            for row in layer {
                for key in row {
                    assert_eq!(key.chars().count(), 1, "{key:?}");
                }
            }
        }
    }

    #[test]
    fn typing_shifting_and_deleting() {
        let mut k = Keyboard::default();
        k.step(0, 1);
        k.type_key();
        k.next_layer();
        k.type_key();
        k.space();
        k.next_layer();
        k.step(0, -1);
        k.type_key();
        assert_eq!(k.text, "qQ !");
        assert!(k.delete());
        assert_eq!(k.text, "qQ ");
        k.next_layer();
        assert_eq!(k.layer, 0, "the layers do not wrap");
    }

    #[test]
    fn a_password_stops_at_the_longest_wpa_takes() {
        let mut k = Keyboard::default();
        for _ in 0..100 {
            k.type_key();
        }
        assert_eq!(k.text.chars().count(), PASSWORD_MAX);
    }
}
