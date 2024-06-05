use clap::ArgAction;

pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse_cli() -> clap::ArgMatches {
  clap::Command::new("Calcit")
    .version(CALCIT_VERSION)
    .author("Jon Chen. <jiyinyiyong@gmail.com>")
    .about("Calcit Scripting Language")
    .arg(
      clap::Arg::new("once")
        .help("skip watching mode, just run once")
        .short('1')
        .long("once")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("emit-js")
        .help("emit JavaScript rather than interpreting")
        .long("emit-js")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("emit-ir")
        .help("emit Cirru EDN representation of program to program-ir.cirru")
        .long("emit-ir")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("disable-stack")
        .help("disable stack trace for errors")
        .long("disable-stack")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("skip-arity-check")
        .help("skip arity check in js codegen")
        .long("skip-arity-check")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("eval")
        .help("evaluate a snippet")
        .short('e')
        .long("eval")
        .num_args(1),
    )
    .arg(
      clap::Arg::new("dep")
        .help("inject dependency")
        .short('d')
        .long("dep")
        .action(ArgAction::Append),
    )
    .arg(
      clap::Arg::new("emit-path")
        .help("specify another directory for js, rather than `js-out/`")
        .long("emit-path")
        .num_args(1),
    )
    .arg(
      clap::Arg::new("init-fn")
        .help("specify `init_fn` which is main function")
        .long("init-fn")
        .num_args(1),
    )
    .arg(
      clap::Arg::new("reload-fn")
        .help("specify `reload_fn` which is called after hot reload")
        .long("reload-fn")
        .num_args(1),
    )
    .arg(clap::Arg::new("entry").help("specify with config entry").long("entry").num_args(1))
    .arg(
      clap::Arg::new("watch-dir")
        .help("specify a path to watch assets changes")
        .long("watch-dir")
        .num_args(1),
    )
    .arg(
      clap::Arg::new("reload-libs")
        .help("force reloading libs data during code reload")
        .long("reload-libs")
        .num_args(0),
    )
    .arg(
      clap::Arg::new("input")
        .help("entry file path")
        .default_value("compact.cirru")
        .index(1),
    )
    .get_matches()
}
