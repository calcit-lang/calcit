pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse_cli() -> clap::ArgMatches {
  clap::Command::new("Calcit")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Scripting Language")
    .arg(
      clap::Arg::new("once")
        .help("disable watching mode")
        .short('1')
        .long("once")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("emit-js")
        .help("emit js rather than interpreting")
        .long("emit-js")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("emit-ir")
        .help("emit EDN representation of program to program-ir.cirru")
        .long("emit-ir")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("eval")
        .help("eval a snippet")
        .short('e')
        .long("eval")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("dep")
        .help("add dependency")
        .short('d')
        .long("dep")
        .multiple_occurrences(true)
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("emit-path")
        .help("emit directory for js, defaults to `js-out/`")
        .long("emit-path")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("init-fn")
        .help("overwrite `init_fn`")
        .long("init-fn")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("reload-fn")
        .help("overwrite `reload_fn`")
        .long("reload-fn")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("entry")
        .help("overwrite with config entry")
        .long("entry")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("watch-dir")
        .help("a folder of assets that also being watched")
        .long("watch-dir")
        .takes_value(true),
    )
    .arg(
      clap::Arg::new("reload-libs")
        .help("reload libs data during code reload")
        .long("reload-libs")
        .takes_value(false),
    )
    .arg(
      clap::Arg::new("input")
        .help("entry file path, defaults to compact.cirru")
        .default_value("compact.cirru")
        .index(1),
    )
    .get_matches()
}
