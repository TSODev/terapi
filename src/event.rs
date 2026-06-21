use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::{Duration, Instant};

/// Terminal events.
#[derive(Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
}

/// Terminal event handler.
pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        Self {
            tick_rate: Duration::from_millis(tick_rate_ms),
        }
    }

    pub fn next(&self) -> Result<Event> {
        let deadline = Instant::now() + self.tick_rate;

        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());

            if event::poll(timeout)? {
                if let CrosstermEvent::Key(key) = event::read()? {
                    return Ok(Event::Key(key));
                }
            } else {
                return Ok(Event::Tick);
            }
        }
    }
}
