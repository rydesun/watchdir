use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct Printer {
    timeout: Duration,
    counter: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Printer {
    pub fn new(timeout: u64) -> Self {
        let counter = Arc::new(Mutex::new(HashSet::new()));
        Self { timeout: Duration::from_millis(timeout), counter }
    }

    pub fn should_print(&mut self, path: &Path) -> bool {
        if self.timeout.is_zero() {
            true
        } else if self.counter.lock().unwrap().contains(path) {
            false
        } else {
            let timeout = self.timeout;
            let path = path.to_owned();
            let counter = Arc::clone(&self.counter);

            counter.lock().unwrap().insert(path.to_owned());
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                counter.lock().unwrap().remove(&path);
            });
            true
        }
    }
}
