use slot_ui::{scroll_offset, search_face, search_hit_slot};

#[test]
fn a_line_that_fits_never_scrolls() {
    assert_eq!(scroll_offset(300.0, 656.0, 5_000), 0.0);
}

#[test]
fn a_long_line_waits_slides_to_its_end_waits_and_starts_over() {
    let (w, box_w) = (856.0, 656.0);
    assert_eq!(scroll_offset(w, box_w, 0), 0.0);
    assert_eq!(scroll_offset(w, box_w, 1_000), 0.0, "waits first");
    let mid = scroll_offset(w, box_w, 1_200 + 1_000);
    assert!(mid > 0.0 && mid < 200.0, "slides, got {mid}");
    let slide_ms = (200.0 / 0.06) as u64;
    assert_eq!(
        scroll_offset(w, box_w, 1_200 + slide_ms + 500),
        200.0,
        "rests at the end"
    );
    let cycle = 2_400 + slide_ms;
    assert_eq!(scroll_offset(w, box_w, cycle + 100), 0.0, "starts over");
}

#[test]
fn a_long_result_keeps_the_full_text_size_instead_of_shrinking() {
    let short = search_face(search_hit_slot(0), "Fusion (GBA)", true);
    let long = search_face(
        search_hit_slot(0),
        "Legend of Zelda, The - A Link to the Past & Four Swords (USA) (GBA)",
        true,
    );
    assert_eq!(short.h, long.h);
    assert!(long.w > 656, "the long name was squeezed to {} px", long.w);
}
