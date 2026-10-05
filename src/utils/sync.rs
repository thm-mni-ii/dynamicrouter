use std::sync::Arc;
use tokio::sync::Mutex;

pub struct InFlightCounter {
    counter: Arc<Mutex<u64>>
}

impl InFlightCounter {
    pub async fn increment(&self) -> InFlightCounterHandle {
        let mut locked = self.counter.lock().await;
        *locked += 1;
        InFlightCounterHandle {counter: self.counter.clone()}
    }
    pub async fn get_count(&self) -> u64 {
        *self.counter.lock().await
    }
}

pub struct InFlightCounterHandle {
    counter: Arc<Mutex<u64>>
}

impl Drop for InFlightCounterHandle {
    fn drop(&mut self) {
        let counter = self.counter.clone();
        tokio::task::spawn(async move {
            let mut locked = counter.lock().await;
            *locked -= 1;
        });
    }
}
