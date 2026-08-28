use std::sync::atomic::{self, AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, LazyLock, Mutex};
use std::{thread, time};

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_WAKE: LazyLock<(Mutex<()>, Condvar)> = LazyLock::new(|| (Mutex::new(()), Condvar::new()));

pub fn reset_shutdown() {
  SHUTDOWN_REQUESTED.store(false, Ordering::Release);
}

pub fn request_shutdown() {
  // Serialize the predicate change with condvar waiters. Without this lock a
  // waiter can observe `false`, then miss the notification just before it
  // enters `wait_timeout_while`.
  let _guard = SHUTDOWN_WAKE.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  SHUTDOWN_REQUESTED.store(true, Ordering::Release);
  SHUTDOWN_WAKE.1.notify_all();
}

pub fn shutdown_requested() -> bool {
  SHUTDOWN_REQUESTED.load(Ordering::Acquire)
}

pub fn wait_for_shutdown(timeout: time::Duration) -> bool {
  if shutdown_requested() {
    return true;
  }
  let guard = SHUTDOWN_WAKE.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
  let _guard = SHUTDOWN_WAKE
    .1
    .wait_timeout_while(guard, timeout, |_| !shutdown_requested())
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  shutdown_requested()
}

pub fn exit_when_cleared() {
  let delay = time::Duration::from_millis(40);

  // keep looping until remaining task size 0
  loop {
    if TASK_COUNT.load(atomic::Ordering::Relaxed) == 0 {
      break;
    } else {
      thread::sleep(delay);
    }
  }
}

/// by default, watcher adds 1 task
pub fn count_pending_tasks() -> usize {
  TASK_COUNT.load(atomic::Ordering::Relaxed)
}

pub fn track_task_add() {
  TASK_COUNT.fetch_add(1, atomic::Ordering::SeqCst);
}

pub fn track_task_release() {
  TASK_COUNT.fetch_sub(1, atomic::Ordering::SeqCst);
}
