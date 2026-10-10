use calcit_native_ffi::{CalcitFfiAsyncHostV1, CalcitFfiAsyncTaskV1, event_kind, status, task_flags, task_kind};

calcit_native_ffi::export_async_abi_v1!();

fn enqueue(host: CalcitFfiAsyncHostV1, task: u64, kind: u32, payload: &[u8]) -> i32 {
  // SAFETY: the fixture only uses the host table while its task is active;
  // the host copies this payload before returning from enqueue.
  unsafe { host.enqueue.unwrap()(host.context, task, kind, 0, payload.as_ptr(), payload.len()) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn emit_calcit_ffi_async_v1(
  _request: *const u8,
  _len: usize,
  task: *const CalcitFfiAsyncTaskV1,
  host: *const CalcitFfiAsyncHostV1,
) -> i32 {
  // SAFETY: the Calcit host supplies readable protocol-v1 descriptors.
  let (task, host) = unsafe { (*task, *host) };
  let result = enqueue(host, task.handle, event_kind::EMIT, b"[]");
  if result != status::OK {
    return result;
  }
  enqueue(host, task.handle, event_kind::COMPLETE, b"&unit")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fail_calcit_ffi_async_v1(
  _request: *const u8,
  _len: usize,
  task: *const CalcitFfiAsyncTaskV1,
  host: *const CalcitFfiAsyncHostV1,
) -> i32 {
  // SAFETY: the Calcit host supplies readable protocol-v1 descriptors.
  let (task, host) = unsafe { (*task, *host) };
  enqueue(host, task.handle, event_kind::FAIL, b"{} (:code :fixture-failure)")
}

struct IdleTask {
  host: CalcitFfiAsyncHostV1,
  fail: bool,
}

unsafe extern "C" fn cancel_idle(context: u64, handle: u64, _reason: *const u8, _len: usize) -> i32 {
  // SAFETY: configure_task owns this allocation until the host's one cancel.
  let task = unsafe { Box::from_raw(context as *mut IdleTask) };
  eprintln!("fixture-idle-task-cancelled");
  if task.fail {
    enqueue(task.host, handle, event_kind::FAIL, b"{} (:code :shutdown-fixture-failure)")
  } else {
    enqueue(task.host, handle, event_kind::COMPLETE, b"&unit")
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn idle_calcit_ffi_async_v1(
  _request: *const u8,
  _len: usize,
  task: *const CalcitFfiAsyncTaskV1,
  host: *const CalcitFfiAsyncHostV1,
) -> i32 {
  // SAFETY: the Calcit host supplies readable protocol-v1 descriptors.
  let (task, host) = unsafe { (*task, *host) };
  start_idle(task, host, false, task_kind::STREAM, task_flags::SERIAL_EVENTS)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn idle_fail_calcit_ffi_async_v1(
  _request: *const u8,
  _len: usize,
  task: *const CalcitFfiAsyncTaskV1,
  host: *const CalcitFfiAsyncHostV1,
) -> i32 {
  // SAFETY: the Calcit host supplies readable protocol-v1 descriptors.
  let (task, host) = unsafe { (*task, *host) };
  start_idle(task, host, true, task_kind::STREAM, task_flags::SERIAL_EVENTS)
}

fn start_idle(task: CalcitFfiAsyncTaskV1, host: CalcitFfiAsyncHostV1, fail: bool, kind: u32, flags: u32) -> i32 {
  let context = Box::into_raw(Box::new(IdleTask { host, fail })) as u64;
  // SAFETY: the callback retains a copy of the host table and acknowledges
  // cancellation synchronously while the owning runtime is still live.
  let result = unsafe { host.configure_task.unwrap()(host.context, task.handle, kind, flags, context, Some(cancel_idle)) };
  if result != status::OK {
    // SAFETY: a rejected configuration never takes ownership of context.
    unsafe { drop(Box::from_raw(context as *mut IdleTask)) };
  }
  result
}

unsafe extern "C" fn record_response(_context: u64, _handle: u64, outcome: u32, _payload: *const u8, _len: usize) -> i32 {
  eprintln!("fixture-response-outcome={outcome}");
  status::OK
}

#[unsafe(no_mangle)]
unsafe extern "C" fn server_calcit_ffi_async_v1(
  _request: *const u8,
  _len: usize,
  task: *const CalcitFfiAsyncTaskV1,
  host: *const CalcitFfiAsyncHostV1,
) -> i32 {
  // SAFETY: the Calcit host supplies readable protocol-v1 descriptors.
  let (task, host) = unsafe { (*task, *host) };
  let configured = start_idle(
    task,
    host,
    false,
    task_kind::SERVER,
    task_flags::SERIAL_EVENTS | task_flags::REQUIRES_RESPONSE,
  );
  if configured != status::OK {
    return configured;
  }
  let mut response = 0;
  // SAFETY: the resolver is static and output storage remains live for the
  // call. The owning task remains active until the Calcit callback cancels it.
  let opened = unsafe { host.open_response.unwrap()(host.context, task.handle, 0, 5000, Some(record_response), &mut response) };
  if opened != status::OK {
    return opened;
  }
  // SAFETY: the host copies this static request and owns the response handle.
  unsafe { host.enqueue.unwrap()(host.context, task.handle, event_kind::EMIT, response, b"[]".as_ptr(), 2) }
}
