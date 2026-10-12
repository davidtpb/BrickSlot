use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use slot_cheevos::{Console, Event};
use slot_retro::MemoryRegion;
use slot_store::ini;

pub const LOGIN_FILE: &str = "Config/retroachievements.txt";

const APP: &str = concat!("brickslot/", env!("CARGO_PKG_VERSION"));

static ON: AtomicBool = AtomicBool::new(false);

pub fn enable(on: bool) {
    if on {
        slot_cheevos::start(APP);
    } else if ON.load(Ordering::Relaxed) {
        slot_cheevos::logout();
    }
    ON.store(on, Ordering::Relaxed);
}

pub fn set_hardcore(on: bool) {
    slot_cheevos::set_hardcore(on);
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

pub enum Login {
    Started,
    NoAccount,
}

pub fn login(root: &Path) -> Login {
    let file = ini::read(root, LOGIN_FILE);
    let get = |k: &str| file.get(k).filter(|v| !v.is_empty()).cloned();
    let Some(user) = get("username") else {
        return Login::NoAccount;
    };
    match (get("token"), get("password")) {
        (Some(token), _) => slot_cheevos::login_token(&user, &token),
        (None, Some(password)) => slot_cheevos::login_password(&user, &password),
        (None, None) => return Login::NoAccount,
    }
    Login::Started
}

/// Keeps the token the server handed back, so the password does not have to
/// sit on the card.
pub fn remember(root: &Path, user: &str, token: &str) -> std::io::Result<()> {
    let text = format!(
        "# Written by brickslot after logging in. To log in as someone else,\n\
         # replace this with username=... and password=... lines.\n\
         username = {user}\ntoken = {token}\n"
    );
    slot_store::atomic_write(&root.join(LOGIN_FILE), text.as_bytes())
}

pub fn console_of(rom: &Path) -> Option<Console> {
    let ext = rom.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "gba" => Console::Gba,
        "gbc" => Console::Gbc,
        "gb" => Console::Gb,
        "zip" => {
            let name = slot_store::rom::zip_rom_name(rom)?;
            let inner = Path::new(&name).extension()?.to_str()?.to_ascii_lowercase();
            match inner.as_str() {
                "gba" => Console::Gba,
                "gbc" => Console::Gbc,
                "gb" => Console::Gb,
                _ => return None,
            }
        }
        _ => return None,
    })
}

/// While this lives, the loaded game's achievements are watched. Dropping it
/// lets go of the core's memory, so it has to go before the core does.
pub struct Session;

impl Drop for Session {
    fn drop(&mut self) {
        slot_cheevos::unload_game();
    }
}

pub fn begin(rom: &Path, regions: Vec<MemoryRegion>) -> Option<Session> {
    if !enabled() {
        return None;
    }
    let console = console_of(rom)?;
    if regions.is_empty() {
        eprintln!("slot: cheevos: the core shares no memory map");
        return None;
    }
    let bytes = match slot_store::rom::open(rom) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("slot: cheevos: {}: {e}", rom.display());
            return None;
        }
    };
    slot_cheevos::set_memory(console, move |real, buf| {
        regions
            .iter()
            .map(|r| unsafe { r.read(real, buf) })
            .find(|&n| n > 0)
            .unwrap_or(0)
    });
    slot_cheevos::load_game(&bytes);
    Some(Session)
}

pub fn frame() {
    if enabled() {
        slot_cheevos::do_frame();
    }
}

pub fn idle() {
    if enabled() {
        slot_cheevos::idle();
    }
}

pub fn take() -> Vec<Event> {
    match enabled() {
        true => slot_cheevos::take_events(),
        false => Vec::new(),
    }
}

/// What the banner says for an event, if anything.
pub fn words(event: &Event) -> Option<(String, String)> {
    Some(match event {
        Event::LoggedIn { user, .. } => {
            ("RetroAchievements".into(), format!("Logged in as {user}"))
        }
        Event::LoginFailed(why) => ("RetroAchievements login failed".into(), why.clone()),
        Event::GameLoaded {
            title, total: 0, ..
        } => (title.clone(), "No achievements yet".into()),
        Event::GameLoaded {
            title,
            unlocked,
            total,
        } => (title.clone(), format!("{unlocked} of {total} achievements")),
        Event::GameFailed(why) => ("No achievements found".into(), why.clone()),
        Event::Unlocked {
            title,
            description,
            points,
        } => (format!("Unlocked: {title} ({points})"), description.clone()),
        Event::Mastered { title } => ("Mastered!".into(), title.clone()),
        Event::ServerError(why) => ("RetroAchievements error".into(), why.clone()),
        Event::Disconnected => (
            "RetroAchievements offline".into(),
            "Unlocks will be sent later".into(),
        ),
        Event::Reconnected => return None,
    })
}
