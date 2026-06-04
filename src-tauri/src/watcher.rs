use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub struct SessionWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Event>,
}

impl SessionWatcher {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

        watcher.configure(Config::default().with_poll_interval(Duration::from_millis(500)))?;

        for path in &paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
            }
        }

        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub fn try_recv(&self) -> Option<notify::Event> {
        self.rx.try_recv().ok()
    }
}
