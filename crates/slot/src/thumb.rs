use slot_retro::{GBA_H, GBA_W};

pub fn png(xrgb8888: &[u8]) -> Option<Vec<u8>> {
    let n = (GBA_W * GBA_H) as usize;
    if xrgb8888.len() < n * 4 {
        return None;
    }
    let mut rgb = Vec::with_capacity(n * 3);
    for px in xrgb8888[..n * 4].chunks_exact(4) {
        rgb.extend_from_slice(&[px[2], px[1], px[0]]);
    }

    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, GBA_W, GBA_H);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(&rgb).ok()?;
    writer.finish().ok()?;
    Some(out)
}

/// The middle `w` x `h` of a frame PNG, where a Game Boy picture sits inside
/// the GBA-sized frame. The PNG comes back untouched if it is no bigger.
pub fn crop_png(frame: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(frame));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.width <= w && info.height <= h {
        return Some(frame.to_vec());
    }
    let bpp = info.line_size / info.width as usize;
    let (x0, y0) = ((info.width - w) / 2, (info.height - h) / 2);
    let mut cut = Vec::with_capacity((w * h) as usize * bpp);
    for y in y0..y0 + h {
        let at = y as usize * info.line_size + x0 as usize * bpp;
        cut.extend_from_slice(&buf[at..at + w as usize * bpp]);
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(info.color_type);
    enc.set_depth(info.bit_depth);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(&cut).ok()?;
    writer.finish().ok()?;
    Some(out)
}
