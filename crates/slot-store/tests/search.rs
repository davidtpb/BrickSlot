use slot_store::{fold, rank};

#[test]
fn accents_case_and_punctuation_are_ignored() {
    assert_eq!(
        fold("Pokémon - FireRed Version (USA, Europe)"),
        "pokemon firered version usa europe"
    );
    assert_eq!(fold("Legend of Zelda, The"), "legend of zelda the");
}

#[test]
fn a_name_starting_with_the_query_comes_before_one_that_only_holds_it() {
    let q = fold("pok");
    assert_eq!(rank(&q, "Pokémon Emerald"), Some(0));
    assert_eq!(rank(&q, "Jurassic PokéPark"), Some(1));
    assert_eq!(rank(&fold("emer"), "Pokémon Emerald"), Some(1));
    assert_eq!(rank(&fold("merald"), "Pokémon Emerald"), Some(2));
    assert_eq!(rank(&fold("zelda"), "Pokémon Emerald"), None);
}

#[test]
fn every_word_of_the_query_has_to_be_in_the_name() {
    assert_eq!(
        rank(&fold("fire red"), "Pokémon - FireRed Version"),
        Some(1)
    );
    assert_eq!(rank(&fold("fire blue"), "Pokémon - FireRed Version"), None);
    assert_eq!(rank(&fold(""), "anything"), None);
}
