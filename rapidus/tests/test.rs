extern crate rapidus;
use rapidus::executer::compile_and_run_file;
use rapidus::executer::GenericValue;

#[test]
fn rapidus_fibo() {
  let res = compile_and_run_file("tests/fibo.js").unwrap();
  let expected = 55;
  if res != GenericValue::Int32(expected) {
    panic!("expected Int32({}), but {:?}", expected, res);
  }
}