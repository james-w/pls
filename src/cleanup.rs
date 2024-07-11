use log::debug;

#[derive(Default)]
pub struct CleanupManager {
    pub(super) cleanups: Vec<(String, Box<dyn FnOnce() + Send + Sync>)>,
}

impl CleanupManager {
    pub fn new() -> Self {
        CleanupManager::default()
    }

    pub fn push_cleanup<F>(&mut self, name: String, cleanup: F)
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        self.cleanups.push((name, Box::new(cleanup)));
    }

    pub fn pop_cleanup(&mut self) {
        self.cleanups.pop();
    }

    pub fn run_cleanups(&mut self) {
        for (name, cleanup) in self.cleanups.drain(..) {
            debug!("Running cleanup <{}>", name);
            cleanup();
        }
    }
}
