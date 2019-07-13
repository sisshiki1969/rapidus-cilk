extern crate cilk;
extern crate clap;
extern crate libc;
extern crate rustyline;
use clap::{App, Arg};

const VERSION_STR: &'static str = env!("CARGO_PKG_VERSION");

pub fn main() {
  let app = App::new("Rapidus")
    .version(VERSION_STR)
    .author("uint256_t")
    .about("A toy JavaScript engine")
    .arg(Arg::with_name("file").help("Input file name").index(1));
  let app_matches = app.clone().get_matches();
  let file_name = match app_matches.value_of("file") {
    Some(file_name) => file_name,
    None => {
      return;
    }
  };

  rapidus::executer::compile_and_run_file(file_name).unwrap();
}
