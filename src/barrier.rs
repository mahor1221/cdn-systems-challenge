// A combination of [`std::sync::Barrier`] and [`crossbeam::sync::WaitGroup`]

use std::{
  fmt,
  sync::{Arc, Condvar, Mutex},
};

pub struct Barrier {
  inner: Arc<Inner>,
}

struct Inner {
  lock: Mutex<BarrierState>,
  cvar: Condvar,
  num_threads: Mutex<usize>,
}

// The inner state of a double barrier
struct BarrierState {
  count: usize,
  generation_id: usize,
}

/// A `BarrierWaitResult` is returned by [`Barrier::wait()`] when all threads
/// in the [`Barrier`] have rendezvoused.
///
/// # Examples
///
/// ```
/// use std::sync::Barrier;
///
/// let barrier = Barrier::new(1);
/// let barrier_wait_result = barrier.wait();
/// ```
pub struct BarrierWaitResult(bool);

impl Barrier {
  /// Creates a new barrier and returns the single reference to it.
  ///
  /// # Examples
  ///
  /// ```
  /// use std::sync::Barrier;
  ///
  /// let barrier = Barrier::new();
  /// ```
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Blocks the current thread until all threads have rendezvoused here.
  ///
  /// Barriers are re-usable after all threads have rendezvoused once, and can
  /// be used continuously.
  ///
  /// A single (arbitrary) thread will receive a [`BarrierWaitResult`] that
  /// returns `true` from [`BarrierWaitResult::is_leader()`] when returning
  /// from this function, and all other threads will receive a result that
  /// will return `false` from [`BarrierWaitResult::is_leader()`].
  ///
  /// # Examples
  ///
  /// ```
  /// use std::sync::{Arc, Barrier};
  /// use std::thread;
  ///
  /// let n = 10;
  /// let mut handles = Vec::with_capacity(n);
  /// let barrier = Arc::new(Barrier::new(n));
  /// for _ in 0..n {
  ///     let c = Arc::clone(&barrier);
  ///     // The same messages will be printed together.
  ///     // You will NOT see any interleaving.
  ///     handles.push(thread::spawn(move|| {
  ///         println!("before wait");
  ///         c.wait();
  ///         println!("after wait");
  ///     }));
  /// }
  /// // Wait for other threads to finish.
  /// for handle in handles {
  ///     handle.join().unwrap();
  /// }
  /// ```
  pub fn wait(&self) -> BarrierWaitResult {
    let mut lock = self.inner.lock.lock().unwrap();
    let local_gen = lock.generation_id;
    lock.count += 1;
    if lock.count < *self.inner.num_threads.lock().unwrap() {
      let _guard = self
        .inner
        .cvar
        .wait_while(lock, |state| local_gen == state.generation_id)
        .unwrap();
      BarrierWaitResult(false)
    } else {
      lock.count = 0;
      lock.generation_id = lock.generation_id.wrapping_add(1);
      self.inner.cvar.notify_all();
      BarrierWaitResult(true)
    }
  }
}

impl Default for Barrier {
  fn default() -> Self {
    Self {
      inner: Arc::new(Inner {
        lock: Mutex::new(BarrierState {
          count: 0,
          generation_id: 0,
        }),
        cvar: Condvar::new(),
        num_threads: Mutex::new(1),
      }),
    }
  }
}

impl fmt::Debug for Barrier {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let num_threads: &usize = &*self.inner.num_threads.lock().unwrap();
    f.debug_struct("Barrier")
      .field("num_threads", num_threads)
      .finish_non_exhaustive()
  }
}

impl Drop for Barrier {
  fn drop(&mut self) {
    let mut count = self.inner.num_threads.lock().unwrap();
    *count -= 1;

    if *count == 0 {
      self.inner.cvar.notify_all();
    }
  }
}

impl Clone for Barrier {
  fn clone(&self) -> Self {
    let mut count = self.inner.num_threads.lock().unwrap();
    *count += 1;

    Self {
      inner: self.inner.clone(),
    }
  }
}

impl fmt::Debug for BarrierWaitResult {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("BarrierWaitResult")
      .field("is_leader", &self.is_leader())
      .finish()
  }
}

impl BarrierWaitResult {
  /// Returns `true` if this thread is the "leader thread" for the call to
  /// [`Barrier::wait()`].
  ///
  /// Only one thread will have `true` returned from their result, all other
  /// threads will have `false` returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use std::sync::Barrier;
  ///
  /// let barrier = Barrier::new(1);
  /// let barrier_wait_result = barrier.wait();
  /// println!("{:?}", barrier_wait_result.is_leader());
  /// ```
  #[must_use]
  pub fn is_leader(&self) -> bool {
    self.0
  }
}
