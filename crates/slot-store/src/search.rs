use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// A name reduced for matching: accents dropped, lower case, and anything
/// that is not a letter or digit turned into single spaces.
pub fn fold(name: &str) -> String {
    let plain: String = name
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How well a game's name answers a folded query: 0 when the name starts with
/// it, 1 when one of its words does, 2 when every query word is somewhere in
/// it. None when it does not match at all.
pub fn rank(folded_query: &str, name: &str) -> Option<u8> {
    let words: Vec<&str> = folded_query.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let name = fold(name);
    if !words.iter().all(|w| name.contains(w)) {
        return None;
    }
    if name.starts_with(folded_query) {
        Some(0)
    } else if name.split(' ').any(|w| w.starts_with(words[0])) {
        Some(1)
    } else {
        Some(2)
    }
}
