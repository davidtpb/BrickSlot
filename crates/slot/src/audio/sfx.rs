const INSERT: &[u8] = include_bytes!("../../assets/insert.pcm");
const EJECT: &[u8] = include_bytes!("../../assets/eject.pcm");
const ASSET_HZ: f32 = 48_000.0;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Sfx {
    Insert,
    Eject,
    /// A soft tick as the cursor moves through a menu.
    Move,
    /// A rising blip as a choice is taken.
    Select,
    /// A falling blip on the way back out.
    Back,
}

impl Sfx {
    pub fn lead(self) -> f32 {
        match self {
            Sfx::Insert => 0.097,
            Sfx::Eject => 0.021,
            Sfx::Move | Sfx::Select | Sfx::Back => 0.0,
        }
    }

    pub fn tail(self) -> f32 {
        let pcm = match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            ui => return ui.length(),
        };
        pcm.len() as f32 / 2.0 / ASSET_HZ - self.lead()
    }

    pub fn render(self, sample_rate: u32) -> Vec<i16> {
        let pcm = match self {
            Sfx::Insert => INSERT,
            Sfx::Eject => EJECT,
            ui => return ui.synth(sample_rate),
        };
        let mut out = Vec::with_capacity(pcm.len());
        for v in resampled(pcm, sample_rate) {
            let s = v.clamp(-32768.0, 32767.0) as i16;
            out.push(s);
            out.push(s);
        }
        out
    }
}

fn resampled(pcm: &[u8], sample_rate: u32) -> Vec<f32> {
    let src: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32)
        .collect();
    if sample_rate == ASSET_HZ as u32 || src.is_empty() {
        return src;
    }
    let ratio = sample_rate as f32 / ASSET_HZ;
    let n = (src.len() as f32 * ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f32 / ratio;
            let a = x as usize;
            let f = x - a as f32;
            let b = (a + 1).min(src.len() - 1);
            src[a] * (1.0 - f) + src[b] * f
        })
        .collect()
}

impl Sfx {
    fn length(self) -> f32 {
        match self {
            Sfx::Move => 0.03,
            _ => 0.07,
        }
    }

    /// The menu sounds are made here rather than recorded: a sine whose pitch
    /// glides from one note to another under a quick fade.
    fn synth(self, sample_rate: u32) -> Vec<i16> {
        let (from, to, level, decay) = match self {
            Sfx::Move => (1760.0, 1760.0, 0.10, 0.008),
            Sfx::Select => (880.0, 1320.0, 0.12, 0.022),
            Sfx::Back => (1320.0, 740.0, 0.11, 0.022),
            Sfx::Insert | Sfx::Eject => return Vec::new(),
        };
        let rate = sample_rate.max(1) as f32;
        let n = (self.length() * rate) as usize;
        let mut phase = 0.0f32;
        let mut out = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f32 / rate;
            let k = t / self.length();
            let pitch = from + (to - from) * k.min(1.0);
            phase += std::f32::consts::TAU * pitch / rate;
            let attack = (t / 0.002).min(1.0);
            let env = attack * (-t / decay).exp();
            let v = (phase.sin() * env * level * 32767.0) as i16;
            out.push(v);
            out.push(v);
        }
        out
    }
}
