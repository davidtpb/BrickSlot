//! RetroAchievements for slot, through rcheevos's rc_client.
//!
//! rc_client is one per process, so this is a set of free functions over a
//! single client. Server calls run on their own threads; whatever the client
//! has to say comes back as [`Event`]s for the frontend to show.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Mutex, OnceLock, RwLock};

pub const RC_OK: c_int = 0;

const DONE_LOGIN: c_int = 1;
const DONE_LOAD: c_int = 2;

const EVENT_TRIGGERED: u32 = 1;
const EVENT_GAME_COMPLETED: u32 = 15;
const EVENT_SERVER_ERROR: u32 = 16;
const EVENT_DISCONNECTED: u32 = 17;
const EVENT_RECONNECTED: u32 = 18;

const RETRYABLE_CLIENT_ERROR: c_int = -2;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Console {
    Gb = 4,
    Gba = 5,
    Gbc = 6,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Event {
    LoggedIn {
        user: String,
        token: String,
    },
    LoginFailed(String),
    GameLoaded {
        title: String,
        unlocked: u32,
        total: u32,
    },
    GameFailed(String),
    Unlocked {
        title: String,
        description: String,
        points: u32,
    },
    Mastered {
        title: String,
    },
    ServerError(String),
    Disconnected,
    Reconnected,
}

type Read = Box<dyn Fn(u32, &mut [u8]) -> usize + Send + Sync>;

static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());
static MEMORY: RwLock<Option<(Console, Read)>> = RwLock::new(None);
static CREATED: OnceLock<bool> = OnceLock::new();
static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
static APP: Mutex<String> = Mutex::new(String::new());

extern "C" {
    fn slot_rc_create() -> c_int;
    fn slot_rc_respond(
        callback: *mut c_void,
        callback_data: *mut c_void,
        body: *const c_char,
        len: usize,
        status: c_int,
    );
    fn slot_rc_login_password(user: *const c_char, password: *const c_char);
    fn slot_rc_login_token(user: *const c_char, token: *const c_char);
    fn slot_rc_logout();
    fn slot_rc_user(
        user: *mut c_char,
        user_size: usize,
        token: *mut c_char,
        token_size: usize,
    ) -> c_int;
    fn slot_rc_load_game(data: *const u8, size: usize);
    fn slot_rc_game(title: *mut c_char, size: usize, unlocked: *mut u32, total: *mut u32) -> c_int;
    fn slot_rc_unload_game();
    fn slot_rc_reset();
    fn slot_rc_do_frame();
    fn slot_rc_idle();
    fn slot_rc_user_agent(buffer: *mut c_char, size: usize) -> usize;
    fn slot_rc_real_address(console: u32, address: u32, real: *mut u32) -> u32;
}

fn push(event: Event) {
    EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push_back(event);
}

fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// Creates the client the first time. `app` leads the User-Agent, as
/// RetroAchievements asks of every emulator, e.g. "brickslot/1.0".
pub fn start(app: &str) -> bool {
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = app.to_string();
    *CREATED.get_or_init(|| unsafe { slot_rc_create() } != 0)
}

fn started() -> bool {
    CREATED.get().copied().unwrap_or(false)
}

pub fn login_password(user: &str, password: &str) {
    if started() {
        let (u, p) = (cstring(user), cstring(password));
        unsafe { slot_rc_login_password(u.as_ptr(), p.as_ptr()) };
    }
}

pub fn login_token(user: &str, token: &str) {
    if started() {
        let (u, t) = (cstring(user), cstring(token));
        unsafe { slot_rc_login_token(u.as_ptr(), t.as_ptr()) };
    }
}

pub fn logout() {
    if started() {
        unsafe { slot_rc_logout() };
    }
}

/// Hashes the ROM as RetroAchievements does for Game Boy and GBA (MD5 of the
/// whole file) and asks the server for its achievements.
pub fn load_game(rom: &[u8]) {
    if started() {
        unsafe { slot_rc_load_game(rom.as_ptr(), rom.len()) };
    }
}

pub fn unload_game() {
    if started() {
        unsafe { slot_rc_unload_game() };
    }
    clear_memory();
}

pub fn reset() {
    if started() {
        unsafe { slot_rc_reset() };
    }
}

pub fn do_frame() {
    if started() && MEMORY.read().is_ok_and(|m| m.is_some()) {
        unsafe { slot_rc_do_frame() };
    }
}

pub fn idle() {
    if started() {
        unsafe { slot_rc_idle() };
    }
}

/// `read` copies from the console's own bus address into the buffer and
/// says how many bytes it filled.
pub fn set_memory(
    console: Console,
    read: impl Fn(u32, &mut [u8]) -> usize + Send + Sync + 'static,
) {
    *MEMORY.write().unwrap_or_else(|e| e.into_inner()) = Some((console, Box::new(read)));
}

pub fn clear_memory() {
    *MEMORY.write().unwrap_or_else(|e| e.into_inner()) = None;
}

pub fn take_events() -> Vec<Event> {
    EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect()
}

pub fn real_address(console: Console, address: u32) -> Option<(u32, u32)> {
    let mut real = 0;
    let avail = unsafe { slot_rc_real_address(console as u32, address, &mut real) };
    (avail > 0).then_some((real, avail))
}

pub fn read(address: u32, buf: &mut [u8]) -> usize {
    let memory = MEMORY.read().unwrap_or_else(|e| e.into_inner());
    let Some((console, read)) = memory.as_ref() else {
        return 0;
    };
    let mut filled = 0;
    while filled < buf.len() {
        let Some((real, avail)) = real_address(*console, address + filled as u32) else {
            break;
        };
        let want = (avail as usize).min(buf.len() - filled);
        let got = read(real, &mut buf[filled..filled + want]);
        filled += got;
        if got < want {
            break;
        }
    }
    filled
}

#[no_mangle]
extern "C" fn slot_rc_read(address: u32, buffer: *mut u8, num_bytes: u32) -> u32 {
    if buffer.is_null() || num_bytes == 0 {
        return 0;
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(buffer, num_bytes as usize) };
    read(address, buf) as u32
}

#[no_mangle]
extern "C" fn slot_rc_event(
    kind: u32,
    title: *const c_char,
    description: *const c_char,
    points: u32,
) {
    let (title, description) = (text(title), text(description));
    push(match kind {
        EVENT_TRIGGERED => Event::Unlocked {
            title,
            description,
            points,
        },
        EVENT_GAME_COMPLETED => Event::Mastered { title },
        EVENT_SERVER_ERROR => Event::ServerError(description),
        EVENT_DISCONNECTED => Event::Disconnected,
        EVENT_RECONNECTED => Event::Reconnected,
        _ => return,
    });
}

#[no_mangle]
extern "C" fn slot_rc_done(kind: c_int, result: c_int, message: *const c_char) {
    let message = text(message);
    match (kind, result) {
        (DONE_LOGIN, RC_OK) => {
            let (mut user, mut token) = ([0 as c_char; 256], [0 as c_char; 256]);
            let ok = unsafe { slot_rc_user(user.as_mut_ptr(), 256, token.as_mut_ptr(), 256) };
            if ok != 0 {
                push(Event::LoggedIn {
                    user: text(user.as_ptr()),
                    token: text(token.as_ptr()),
                });
            }
        }
        (DONE_LOGIN, _) => push(Event::LoginFailed(or_unreachable(message))),
        (DONE_LOAD, RC_OK) => {
            let mut title = [0 as c_char; 256];
            let (mut unlocked, mut total) = (0, 0);
            let ok = unsafe { slot_rc_game(title.as_mut_ptr(), 256, &mut unlocked, &mut total) };
            if ok != 0 {
                push(Event::GameLoaded {
                    title: text(title.as_ptr()),
                    unlocked,
                    total,
                });
            }
        }
        (DONE_LOAD, _) => push(Event::GameFailed(or_unreachable(message))),
        _ => {}
    }
}

fn or_unreachable(message: String) -> String {
    match message.is_empty() {
        true => "Could not reach RetroAchievements".to_string(),
        false => message,
    }
}

fn user_agent() -> String {
    let mut buf = [0 as c_char; 128];
    let n = unsafe { slot_rc_user_agent(buf.as_mut_ptr(), buf.len()) };
    let clause = if n > 0 {
        text(buf.as_ptr())
    } else {
        String::new()
    };
    let app = APP.lock().unwrap_or_else(|e| e.into_inner()).clone();
    format!("{app} {clause}").trim().to_string()
}

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(&user_agent())
            .build()
    })
}

fn fetch(url: &str, post: Option<&str>, content_type: &str) -> (c_int, Vec<u8>) {
    let result = match post {
        Some(body) => agent()
            .post(url)
            .set("Content-Type", content_type)
            .send_string(body),
        None => agent().get(url).call(),
    };
    let response = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => {
            eprintln!("slot: cheevos: {url}: {e}");
            return (RETRYABLE_CLIENT_ERROR, Vec::new());
        }
    };
    let status = c_int::from(response.status());
    let mut body = Vec::new();
    if let Err(e) = std::io::Read::read_to_end(&mut response.into_reader(), &mut body) {
        eprintln!("slot: cheevos: {url}: {e}");
        return (RETRYABLE_CLIENT_ERROR, Vec::new());
    }
    (status, body)
}

#[no_mangle]
extern "C" fn slot_rc_http(
    url: *const c_char,
    post_data: *const c_char,
    content_type: *const c_char,
    callback: *mut c_void,
    callback_data: *mut c_void,
) {
    let url = text(url);
    let post = (!post_data.is_null()).then(|| text(post_data));
    let content_type = match text(content_type) {
        t if t.is_empty() => "application/x-www-form-urlencoded".to_string(),
        t => t,
    };
    let (callback, callback_data) = (callback as usize, callback_data as usize);
    let spawned = std::thread::Builder::new()
        .name("slot-cheevos".into())
        .spawn(move || {
            let (status, body) = fetch(&url, post.as_deref(), &content_type);
            unsafe {
                slot_rc_respond(
                    callback as *mut c_void,
                    callback_data as *mut c_void,
                    body.as_ptr() as *const c_char,
                    body.len(),
                    status,
                )
            };
        });
    if let Err(e) = spawned {
        eprintln!("slot: cheevos: {e}");
        unsafe {
            slot_rc_respond(
                callback as *mut c_void,
                callback_data as *mut c_void,
                std::ptr::null(),
                0,
                RETRYABLE_CLIENT_ERROR,
            )
        };
    }
}
