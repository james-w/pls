#[derive(Default)]
pub struct CleanupManager {
    pub(super) cleanups: Vec<Box<dyn FnOnce() + Send + Sync>>,
}

impl CleanupManager {
    pub fn new() -> Self {
        CleanupManager::default()
    }

    pub fn push_cleanup<F>(&mut self, cleanup: F)
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        self.cleanups.push(Box::new(cleanup));
    }

    pub fn pop_cleanup(&mut self) {
        self.cleanups.pop();
    }
}
