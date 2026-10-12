use std::sync::Arc;

use super::ring::{ring_capacity, Ring};
use super::sink::{AudioError, AudioSink};

#[derive(Clone)]
pub struct StubSink {
    ring: Arc<Ring>,
}

impl StubSink {
    pub fn new() -> Self {
        StubSink {
            ring: Arc::new(Ring::new(ring_capacity(32768))),
        }
    }

    pub fn draining() -> Self {
        let sink = StubSink::new();
        let ring = sink.ring.clone();
        std::thread::spawn(move || loop {
            let frames = ring.queued_frames();
            ring.fill(&mut vec![0i16; frames * 2]);
            std::thread::sleep(std::time::Duration::from_millis(2));
        });
        sink
    }

    /// A stand-in for a sound card that plays what it is given at the rate
    /// it was opened at, so what is queued drains as it would on a device.
    pub fn realtime() -> Self {
        let sink = StubSink::new();
        let ring = sink.ring.clone();
        std::thread::spawn(move || {
            let mut last = std::time::Instant::now();
            let mut owed = 0.0f64;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(5));
                let now = std::time::Instant::now();
                owed += now.duration_since(last).as_secs_f64() * f64::from(ring.sample_rate());
                last = now;
                let frames = (owed as usize).min(ring.queued_frames());
                owed -= frames as f64;
                owed = owed.min(f64::from(ring.sample_rate()) / 10.0);
                if frames > 0 {
                    ring.fill(&mut vec![0i16; frames * 2]);
                }
            }
        });
        sink
    }

    pub fn muted(&self) -> bool {
        self.ring.muted()
    }

    pub fn device_read(&self, frames: usize) -> Vec<i16> {
        let mut out = vec![0i16; frames * 2];
        self.ring.fill(&mut out);
        out
    }

    pub fn device_drain(&self) -> usize {
        let frames = self.ring.queued_frames();
        self.ring.fill(&mut vec![0i16; frames * 2]);
        frames
    }
}

impl Default for StubSink {
    fn default() -> Self {
        StubSink::new()
    }
}

impl AudioSink for StubSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        self.ring.reopen(sample_rate);
        Ok(())
    }

    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }
}
