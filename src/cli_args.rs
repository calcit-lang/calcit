pub const CALCIT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse_cli<'a>() -> clap::ArgMatches<'a> {
  clap::App::new("Calcit Runner")
    .version(CALCIT_VERSION)
    .author("Jon. <jiyinyiyong@gmail.com>")
    .about("Calcit Runner")
    .arg(
      clap::Arg::with_name("once")
        .help("disable watching mode")
        .default_value("false")
        .short("1")
        .long("once")
        .takes_value(false),
    )
    .arg(
      clap::Arg::with_name("emit-js")
        .help("emit js rather than interpreting")
        .default_value("false")
        .long("emit-js")
        .takes_value(false),
    )
    .arg(
      clap::Arg::with_name("emit-ir")
        .help("emit JSON representation of program to program-ir.json")
        .default_value("false")
        .long("emit-ir")
        .takes_value(false),
    )
    .arg(
      clap::Arg::with_name("eval")
        .help("eval a snippet")
        .short("e")
        .long("eval")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("dep")
        .help("add dependency")
        .short("d")
        .long("dep")
        .multiple(true)
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("emit-path")
        .help("emit directory for js, defaults to `js-out/`")
        .long("emit-path")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("init-fn")
        .help("overwrite `init_fn`")
        .long("init-fn")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("reload-fn")
        .help("overwrite `reload_fn`")
        .long("reload-fn")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("event-entry")
        .help("entry ns/def for handling events")
        .long("event-entry")
        .default_value("app.main/on-window-event")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("watch-dir")
        .help("a folder of assets that also being watched")
        .long("watch-dir")
        .takes_value(true),
    )
    .arg(
      clap::Arg::with_name("reload-libs")
        .help("reload libs data during code reload")
        .long("reload-libs")
        .takes_value(false),
    )
    .arg(
      clap::Arg::with_name("input")
        .help("entry file path, defaults to compact.cirru")
        .default_value("compact.cirru")
        .index(1),
    )
    .get_matches()
}
