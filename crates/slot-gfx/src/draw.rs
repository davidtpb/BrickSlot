use crate::quad::Quad;
use crate::shaders::{SPRITE_FRAG, SPRITE_VERT, WARP_FRAG, WARP_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TexId(usize);

#[cfg(feature = "test-support")]
impl TexId {
    pub const fn from_raw(id: usize) -> Self {
        TexId(id)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Draw {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        colour: [f32; 4],
    },
    Tex {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
    },
    Turned {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        turn: f32,
    },
    Game,
    Shot {
        tex: TexId,
    },
    /// A horizontal slice of a texture, from `u0` to `u1` across its width
    /// (0 to 1), stretched over the rectangle. Lets text wider than its box
    /// be cut at the box's edge and slid through it.
    Part {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        u0: f32,
        u1: f32,
    },
    /// A texture laid over four corners (top left, top right, bottom left,
    /// bottom right) with perspective: `q` holds one over each corner's
    /// depth. `grey` drains its colour, from 0 (none) to 1 (all).
    Warp {
        corners: [[f32; 2]; 4],
        q: [f32; 4],
        tex: TexId,
        alpha: f32,
        grey: f32,
    },
}

impl Draw {
    /// The box a drawing covers, for layout and tests.
    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        match *self {
            Draw::Rect { x, y, w, h, .. }
            | Draw::Tex { x, y, w, h, .. }
            | Draw::Turned { x, y, w, h, .. }
            | Draw::Part { x, y, w, h, .. } => Some((x, y, w, h)),
            Draw::Warp { corners, .. } => {
                let xs = corners.map(|c| c[0]);
                let ys = corners.map(|c| c[1]);
                let (x0, x1) = (
                    xs.iter().copied().fold(f32::MAX, f32::min),
                    xs.iter().copied().fold(f32::MIN, f32::max),
                );
                let (y0, y1) = (
                    ys.iter().copied().fold(f32::MAX, f32::min),
                    ys.iter().copied().fold(f32::MIN, f32::max),
                );
                Some((x0, y0, x1 - x0, y1 - y0))
            }
            Draw::Game | Draw::Shot { .. } => None,
        }
    }
}

struct Warp {
    prog: gl::types::GLuint,
    u_p01: gl::types::GLint,
    u_p23: gl::types::GLint,
    u_q: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_grey: gl::types::GLint,
}

pub struct Sprites {
    prog: gl::types::GLuint,
    white: gl::types::GLuint,
    textures: Vec<gl::types::GLuint>,
    u_rect: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_turn: gl::types::GLint,
    u_span: gl::types::GLint,
    warp: Warp,
}

impl Sprites {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(SPRITE_VERT, SPRITE_FRAG)?;
        let white = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[255u8; 4]),
        );
        let (u_rect, u_colour, u_turn, u_span);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_colour = crate::gl::uniform_location(prog, "u_colour");
            u_turn = crate::gl::uniform_location(prog, "u_turn");
            u_span = crate::gl::uniform_location(prog, "u_span");
        }
        let warp = unsafe {
            let wp = crate::shaders::program(WARP_VERT, WARP_FRAG)?;
            gl::UseProgram(wp);
            gl::Uniform1i(crate::gl::uniform_location(wp, "u_tex"), 0);
            gl::Uniform2f(
                crate::gl::uniform_location(wp, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            Warp {
                prog: wp,
                u_p01: crate::gl::uniform_location(wp, "u_p01"),
                u_p23: crate::gl::uniform_location(wp, "u_p23"),
                u_q: crate::gl::uniform_location(wp, "u_q"),
                u_colour: crate::gl::uniform_location(wp, "u_colour"),
                u_grey: crate::gl::uniform_location(wp, "u_grey"),
            }
        };
        Ok(Sprites {
            warp,
            prog,
            white,
            textures: Vec::new(),
            u_rect,
            u_colour,
            u_turn,
            u_span,
        })
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::LINEAR)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::NEAREST)
    }

    fn push(&mut self, w: u32, h: u32, rgba: &[u8], filter: gl::types::GLenum) -> TexId {
        let tex = crate::gl::texture(w, h, filter, gl::CLAMP_TO_EDGE, gl::RGBA, Some(rgba));
        self.textures.push(tex);
        TexId(self.textures.len() - 1)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        let Some(tex) = self.textures.get(id.0) else {
            return;
        };
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                crate::gl::internal_format(gl::RGBA),
                w as i32,
                h as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    pub fn source(&self, id: TexId) -> Option<gl::types::GLuint> {
        self.textures.get(id.0).copied()
    }

    pub fn draw(&self, items: &[Draw], quad: &Quad) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        let mut warping = false;
        for item in items {
            if let Draw::Warp {
                corners,
                q,
                tex,
                alpha,
                grey,
            } = *item
            {
                let Some(t) = self.textures.get(tex.0) else {
                    continue;
                };
                let w = &self.warp;
                unsafe {
                    if !warping {
                        gl::UseProgram(w.prog);
                        warping = true;
                    }
                    gl::BindTexture(gl::TEXTURE_2D, *t);
                    let [a, b, c, d] = corners;
                    gl::Uniform4f(w.u_p01, a[0], a[1], b[0], b[1]);
                    gl::Uniform4f(w.u_p23, c[0], c[1], d[0], d[1]);
                    gl::Uniform4f(w.u_q, q[0], q[1], q[2], q[3]);
                    gl::Uniform4f(w.u_colour, 1.0, 1.0, 1.0, alpha);
                    gl::Uniform1f(w.u_grey, grey);
                }
                quad.draw();
                continue;
            }
            if warping {
                unsafe { gl::UseProgram(self.prog) };
                warping = false;
            }
            let mut span = (0.0, 1.0);
            let (x, y, w, h, tex, colour, turn) = match *item {
                Draw::Rect { x, y, w, h, colour } => (x, y, w, h, self.white, colour, 0.0),
                Draw::Part {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    u0,
                    u1,
                } => match self.textures.get(tex.0) {
                    Some(t) => {
                        span = (u0, u1 - u0);
                        (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], 0.0)
                    }
                    None => continue,
                },
                Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], 0.0),
                    None => continue,
                },
                Draw::Turned {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    turn,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], turn),
                    None => continue,
                },
                Draw::Game | Draw::Shot { .. } | Draw::Warp { .. } => continue,
            };
            let (cos, sin) = if turn == 0.0 {
                (1.0, 0.0)
            } else {
                (turn.cos(), turn.sin())
            };
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::Uniform4f(self.u_rect, x, y, w, h);
                gl::Uniform2f(self.u_turn, cos, sin);
                gl::Uniform2f(self.u_span, span.0, span.1);
                gl::Uniform4f(self.u_colour, colour[0], colour[1], colour[2], colour[3]);
            }
            quad.draw();
        }
        unsafe { gl::Disable(gl::BLEND) };
    }
}

impl Drop for Sprites {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
            gl::DeleteTextures(1, &self.white);
            gl::DeleteProgram(self.prog);
            gl::DeleteProgram(self.warp.prog);
        }
    }
}
