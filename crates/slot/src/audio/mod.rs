mod alsa;
#[cfg(feature = "host")]
mod host;
mod ring;
mod sfx;
mod silence;
mod sink;
mod stub;
pub mod volume;

pub use alsa::AlsaSink;
#[cfg(feature = "host")]
pub use host::HostAudio;
pub use ring::{ring_capacity, Ring};
pub use sfx::Sfx;
pub use silence::Silence;
pub use sink::{AudioError, AudioSink};
pub use stub::StubSink;

pub const GBA_HZ: u32 = 32_768;

pub fn opened_sink() -> Box<dyn AudioSink> {
    let mut sink = open_sink();
    if let Err(e) = sink.open(GBA_HZ) {
        eprintln!("slot: audio: {e}");
        if cfg!(feature = "host") {
            // A computer with no sound card still plays: to nowhere, at the
            // pace a card would, so the game keeps time and sounds are
            // queued as they would be.
            eprintln!("slot: audio: playing to nowhere");
            let mut stub = StubSink::realtime();
            let _ = stub.open(GBA_HZ);
            return Box::new(stub);
        }
    }
    sink
}

#[cfg(feature = "host")]
pub fn open_sink() -> Box<dyn AudioSink> {
    if std::env::var_os("SLOT_SILENT").is_some() {
        return Box::new(StubSink::draining());
    }
    Box::new(HostAudio::new())
}

#[cfg(not(feature = "host"))]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(AlsaSink::new())
}
