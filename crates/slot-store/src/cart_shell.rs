use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

pub const CART_SHELL_FILE: &str = "Config/cart_shell.txt";
pub const LABELS_SHELL_FILE: &str = "Labels/cart_shell.txt";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outline {
    Auto,
    Notched,
    Rounded,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShellFinish {
    Solid,
    Clear,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ShellChoice {
    pub outline: Outline,
    pub colour: [u8; 3],
    pub finish: ShellFinish,
}

impl ShellChoice {
    pub fn parse(value: &str) -> Option<ShellChoice> {
        let mut words = value.split_whitespace();
        let outline = match words.next()? {
            "auto" => Outline::Auto,
            "notched" => Outline::Notched,
            "rounded" => Outline::Rounded,
            _ => return None,
        };
        let hex = words.next()?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let colour = [0, 2, 4].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0));
        let finish = match words.next()? {
            "solid" => ShellFinish::Solid,
            "clear" => ShellFinish::Clear,
            "glitter" => ShellFinish::Glitter,
            _ => return None,
        };
        words.next().is_none().then_some(ShellChoice {
            outline,
            colour,
            finish,
        })
    }

    pub fn to_value(&self) -> String {
        let outline = match self.outline {
            Outline::Auto => "auto",
            Outline::Notched => "notched",
            Outline::Rounded => "rounded",
        };
        let finish = match self.finish {
            ShellFinish::Solid => "solid",
            ShellFinish::Clear => "clear",
            ShellFinish::Glitter => "glitter",
        };
        let [r, g, b] = self.colour;
        format!("{outline} {r:02x}{g:02x}{b:02x} {finish}")
    }
}

pub fn key(stem: &str) -> String {
    stem.nfc().collect()
}

pub fn choices(text: &str) -> HashMap<String, ShellChoice> {
    crate::ini::parse(text)
        .into_iter()
        .filter_map(|(stem, value)| Some((key(&stem), ShellChoice::parse(&value)?)))
        .collect()
}

pub fn layered(system: &str, labels: &str) -> HashMap<String, ShellChoice> {
    let mut out = choices(system);
    for (stem, value) in crate::ini::parse(labels) {
        if value.trim() == "auto" {
            out.remove(&key(&stem));
        } else if let Some(choice) = ShellChoice::parse(&value) {
            out.insert(key(&stem), choice);
        }
    }
    out
}

/// Keeps `choice` for the cart `stem` in the card's own shell file, or
/// forgets it so the cart goes back to its own colours.
pub fn save(
    root: &std::path::Path,
    stem: &str,
    choice: Option<ShellChoice>,
) -> std::io::Result<()> {
    let path = root.join(CART_SHELL_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let value = choice.map(|c| c.to_value());
    let out = crate::ini::set(&text, &key(stem), value.as_deref())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic_write(&path, out.as_bytes())
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn a_saved_choice_reads_back_and_forgetting_it_leaves_the_rest() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(
            d.path().join(CART_SHELL_FILE),
            "Other = auto 112233 solid\n",
        )
        .unwrap();
        let pick = ShellChoice {
            outline: Outline::Auto,
            colour: [0x2e, 0xcc, 0x5c],
            finish: ShellFinish::Clear,
        };
        save(d.path(), "Pokémon Emerald", Some(pick)).unwrap();
        let text = std::fs::read_to_string(d.path().join(CART_SHELL_FILE)).unwrap();
        let read = choices(&text);
        assert_eq!(read.get(&key("Pokémon Emerald")), Some(&pick));
        assert!(read.contains_key("Other"));
        save(d.path(), "Pokémon Emerald", None).unwrap();
        let text = std::fs::read_to_string(d.path().join(CART_SHELL_FILE)).unwrap();
        let read = choices(&text);
        assert!(!read.contains_key(&key("Pokémon Emerald")));
        assert!(read.contains_key("Other"));
    }
}
