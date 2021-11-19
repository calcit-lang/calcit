use std::sync::atomic;
use std::sync::atomic::AtomicUsize;
use std::{thread, time};

static TASK_COUNT: AtomicUsize = AtomicUsize::new(0);

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
