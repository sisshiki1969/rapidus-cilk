#![feature(box_patterns)]
#![feature(repeat_generic_slice)]
#![feature(type_ascription)]
pub mod lexer;
pub mod node;
pub mod parser;
pub mod token;
pub mod util;
pub mod executer;

extern crate ansi_term;
extern crate chrono;
extern crate encoding;
extern crate libc;
extern crate libloading;
//extern crate llvm_sys as llvm;
extern crate nanbox;
extern crate nix;
extern crate rand;
extern crate rustc_hash;
extern crate rustyline;
extern crate stopwatch;
// extern crate cpuprofiler;
