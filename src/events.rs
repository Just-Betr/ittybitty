use std::time::Duration;

use crossterm::event::Event;
use tokio::sync::mpsc;

pub fn start_event_thread() -> mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        loop {
            if let Ok(ready) = crossterm::event::poll(Duration::from_millis(200)) {
                if !ready {
                    continue;
                }
                if let Ok(ev) = crossterm::event::read() {
                    let _ = tx.send(ev);
                }
            }
        }
    });
    rx
}
