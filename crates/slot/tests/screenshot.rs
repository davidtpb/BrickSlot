mod common;

use common::{app_playing_with, tmp_root_with_carts, StubSnapshot};
use slot_input::RawEvent::{Down, Up};
use slot_input::{Action, Btn, Gestures};

fn frame_png(w: u32, h: u32) -> Vec<u8> {
    let mut rgb = Vec::new();
    for y in 0..h {
        for x in 0..w {
            rgb.extend_from_slice(&[x as u8, y as u8, 7]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(&rgb).unwrap();
    wr.finish().unwrap();
    out
}

fn size_of(png_bytes: &[u8]) -> (u32, u32) {
    let r = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    (r.info().width, r.info().height)
}

#[test]
fn select_and_x_take_a_screenshot() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Btn::Select), 0).is_empty());
    assert_eq!(g.feed(Down(Btn::X), 50), vec![Action::Screenshot]);
    assert!(
        g.feed(Up(Btn::X), 80).is_empty(),
        "X reached the game as turbo"
    );
}

#[test]
fn a_screenshot_lands_in_screenshots_named_after_the_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let snapshot = StubSnapshot {
        state: vec![9u8; 1024],
        sav: None,
        thumb: Some(frame_png(240, 160)),
        loaded: Default::default(),
    };
    let mut a = app_playing_with(d.path(), "Emerald", Box::new(snapshot));
    a.apply(Action::Screenshot);
    let shots: Vec<_> = std::fs::read_dir(d.path().join("Screenshots"))
        .expect("no Screenshots folder")
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(shots.len(), 1);
    let name = shots[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("Emerald ") && name.ends_with(".png"),
        "{name}"
    );
    assert_eq!(size_of(&std::fs::read(&shots[0]).unwrap()), (240, 160));
    assert_eq!(
        a.take_message().map(|m| m.0),
        Some("Screenshot saved".into())
    );
}

#[test]
fn a_game_boy_screenshot_is_cut_to_the_game_boy_picture() {
    let cropped = slot::thumb::crop_png(&frame_png(240, 160), 160, 144).unwrap();
    assert_eq!(size_of(&cropped), (160, 144));
    let decoder = png::Decoder::new(std::io::Cursor::new(&cropped));
    let mut r = decoder.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    r.next_frame(&mut buf).unwrap();
    assert_eq!(&buf[..3], &[40, 8, 7], "not the middle of the frame");
}
