#![allow(dead_code)]

use std::sync::Arc;

use parking_lot::{Mutex, Condvar};

#[derive(Clone)]
pub struct Semaphore {
    count: Arc<Mutex<usize>>,
    notifier: Arc<Condvar>,
}

pub struct SemaphorePermit<'s> {
    sem: &'s Semaphore,
}

impl<'s> Drop for SemaphorePermit<'s> {
    fn drop(&mut self) {
        self.sem.release();
    }
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            count: Arc::new(Mutex::new(permits)),
            notifier: Arc::new(Condvar::new()),
        }
    }

    pub fn acquire(&self) -> SemaphorePermit {
        let mut lcount = self.count.lock();
        if *lcount == 0 {
            self.notifier.wait(&mut lcount);
        }
        *lcount -= 1;

        SemaphorePermit {
            sem: &self,
        }
    }

    pub fn try_acquire(&self) -> Option<SemaphorePermit> {
        let mut lcount = self.count.lock();
        if *lcount == 0 {
            return None;
        }
        *lcount -= 1;

        Some(SemaphorePermit {
            sem: &self,
        })
    }

    fn release(&self) {
        let mut lcount = self.count.lock();
        *lcount += 1;
        self.notifier.notify_one();
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, sync::atomic::{Ordering, AtomicUsize}, time::Duration};

    fn wait_for(count: &Arc<AtomicUsize>, val: usize) -> bool {
        let mut times = 0;
        loop {
            if count.load(Ordering::SeqCst) == val {
                break true;
            }
            times += 1;
            if times >= 1000 {
                break false;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    #[test]
    fn test_simple_acquire() {
        let sem = Semaphore::new(1);
        let _permit = sem.acquire();
        assert!(true);

        let permit2 = sem.try_acquire();
        assert!(permit2.is_none());
    }

    #[test]
    fn test_simple_try_acquire() {
        let sem = Semaphore::new(1);
        let permit1 = sem.acquire();
        assert!(true);

        let permit2 = sem.try_acquire();
        assert!(permit2.is_none());

        drop(permit1);
        let permit3 = sem.try_acquire();
        assert!(permit3.is_some());
    }

    #[test]
    fn test_acquire_blocking() {
        let sem = Semaphore::new(1);
        let permit = sem.acquire();
        let thread_state = Arc::new(AtomicUsize::new(0));

        let tjoin = {
            let tsem = sem.clone();
            let twait = thread_state.clone();
            thread::spawn(move || {
                twait.fetch_add(1, Ordering::SeqCst);
                tsem.acquire();
                twait.fetch_add(1, Ordering::SeqCst);
            })
        };

        let blocked = wait_for(&thread_state, 1);
        assert!(blocked);

        drop(permit);
        tjoin.join().unwrap();

        assert_eq!(2, thread_state.load(Ordering::SeqCst));
    }

    #[test]
    fn test_aquire_many() {
        let nthreads = 100;
        let sem = Semaphore::new(nthreads);
        let acquired = Arc::new(AtomicUsize::new(0));
        let shutdown = Semaphore::new(1);
        let spermit = shutdown.acquire();

        let mut threads = vec![];
        for _i in 0..nthreads {
            let tsem = sem.clone();
            let tac = acquired.clone();
            let tshut = shutdown.clone();
            let t = thread::spawn(move || {
                let _p1 = tsem.acquire();
                tac.fetch_add(1, Ordering::SeqCst);
                let _sh = tshut.acquire();
                tac.fetch_sub(1, Ordering::SeqCst);
            });
            threads.push(t);
        }

        let all_acquired = wait_for(&acquired, nthreads);
        assert!(all_acquired);
        for t in &threads {
            assert!(!t.is_finished());
        }

        assert!(sem.try_acquire().is_none());

        drop(spermit);
        for t in threads {
            t.join().unwrap();
        }


        let all_released = wait_for(&acquired, 0);
        assert!(all_released);
    }

}
