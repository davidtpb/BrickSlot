fn step<T: Copy + PartialEq>(all: &[T], now: T, right: bool) -> T {
    let n = all.len();
    let at = all.iter().position(|&v| v == now).unwrap_or(0);
    all[if right {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }]
}

fn find<T: Copy>(all: &[T], name: &str, of: impl Fn(T) -> &'static str) -> Option<T> {
    all.iter().copied().find(|&v| of(v) == name)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LightMode {
    Battery,
    Colour,
    Off,
}

impl LightMode {
    pub const ALL: [LightMode; 3] = [LightMode::Battery, LightMode::Colour, LightMode::Off];

    pub fn name(self) -> &'static str {
        match self {
            LightMode::Battery => "battery",
            LightMode::Colour => "colour",
            LightMode::Off => "off",
        }
    }

    pub fn parse(name: &str) -> Option<LightMode> {
        find(&LightMode::ALL, name, LightMode::name)
    }

    pub fn step(self, right: bool) -> LightMode {
        step(&LightMode::ALL, self, right)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LightColour {
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
}

impl LightColour {
    pub const ALL: [LightColour; 9] = [
        LightColour::White,
        LightColour::Red,
        LightColour::Orange,
        LightColour::Yellow,
        LightColour::Green,
        LightColour::Cyan,
        LightColour::Blue,
        LightColour::Purple,
        LightColour::Pink,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LightColour::White => "white",
            LightColour::Red => "red",
            LightColour::Orange => "orange",
            LightColour::Yellow => "yellow",
            LightColour::Green => "green",
            LightColour::Cyan => "cyan",
            LightColour::Blue => "blue",
            LightColour::Purple => "purple",
            LightColour::Pink => "pink",
        }
    }

    pub fn rgb(self) -> u32 {
        match self {
            LightColour::White => 0xFFFFFF,
            LightColour::Red => 0xFF0000,
            LightColour::Orange => 0xFF8C00,
            LightColour::Yellow => 0xFFD700,
            LightColour::Green => 0x00FF00,
            LightColour::Cyan => 0x00FFFF,
            LightColour::Blue => 0x0000FF,
            LightColour::Purple => 0x8A2BE2,
            LightColour::Pink => 0xFF69B4,
        }
    }

    pub fn parse(name: &str) -> Option<LightColour> {
        find(&LightColour::ALL, name, LightColour::name)
    }

    pub fn step(self, right: bool) -> LightColour {
        step(&LightColour::ALL, self, right)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LightEffect {
    Static,
    Breathe,
    Blink,
    Rainbow,
}

impl LightEffect {
    pub const ALL: [LightEffect; 4] = [
        LightEffect::Static,
        LightEffect::Breathe,
        LightEffect::Blink,
        LightEffect::Rainbow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LightEffect::Static => "static",
            LightEffect::Breathe => "breathe",
            LightEffect::Blink => "blink",
            LightEffect::Rainbow => "rainbow",
        }
    }

    pub fn parse(name: &str) -> Option<LightEffect> {
        find(&LightEffect::ALL, name, LightEffect::name)
    }

    pub fn step(self, right: bool) -> LightEffect {
        step(&LightEffect::ALL, self, right)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LightLevel {
    Low,
    Medium,
    High,
}

impl LightLevel {
    pub const ALL: [LightLevel; 3] = [LightLevel::Low, LightLevel::Medium, LightLevel::High];

    pub fn name(self) -> &'static str {
        match self {
            LightLevel::Low => "low",
            LightLevel::Medium => "medium",
            LightLevel::High => "high",
        }
    }

    pub fn percent(self) -> u8 {
        match self {
            LightLevel::Low => 15,
            LightLevel::Medium => 50,
            LightLevel::High => 100,
        }
    }

    pub fn parse(name: &str) -> Option<LightLevel> {
        find(&LightLevel::ALL, name, LightLevel::name)
    }

    pub fn step(self, right: bool) -> LightLevel {
        step(&LightLevel::ALL, self, right)
    }
}
