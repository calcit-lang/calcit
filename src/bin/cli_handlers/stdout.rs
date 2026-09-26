//! Terminal stdout handling for documentation and query commands, not runtime effects.

use std::io::{self, Write};

pub(super) fn write_or_exit(args: std::fmt::Arguments<'_>) {
  let result = {
    let mut output = io::stdout().lock();
    output.write_fmt(args).and_then(|()| output.flush())
  };
  if let Err(error) = result {
    // A reader such as `head` may deliberately stop early. Other I/O failures
    // must remain visible and unsuccessful; do not intercept unrelated panics.
    if error.kind() == io::ErrorKind::BrokenPipe {
      std::process::exit(0);
    }
    let _ = writeln!(io::stderr().lock(), "Error: failed writing CLI stdout: {error}");
    std::process::exit(1);
  }
}

macro_rules! cli_print {
  ($($arg:tt)*) => {
    $crate::cli_handlers::stdout::write_or_exit(format_args!($($arg)*))
  };
}

macro_rules! cli_println {
  () => { $crate::cli_handlers::stdout::write_or_exit(format_args!("\n")) };
  ($($arg:tt)*) => {
    $crate::cli_handlers::stdout::write_or_exit(format_args!("{}\n", format_args!($($arg)*)))
  };
}

pub(super) use cli_print;
pub(super) use cli_println;
