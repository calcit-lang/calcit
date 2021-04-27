pub const CALCIT_VERSION: &str = "0.3.0-a9";

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
      clap::Arg::with_name("input")
        .help("entry file path, defaults to compact.cirru")
        .default_value("compact.cirru")
        .index(1),
    )
    .get_matches()
}
