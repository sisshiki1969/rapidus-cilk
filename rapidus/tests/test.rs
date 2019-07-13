extern crate rapidus;
use rapidus::executer::compile_and_run_file;
use rapidus::executer::GenericValue;

fn test_file(file_name: &str, expected: i32) {
  let res = compile_and_run_file(format!("tests/{}", file_name)).unwrap();
  if res != GenericValue::Int32(expected) {
    panic!("expected Int32({}), but {:?}", expected, res);
  }
}

#[test]
fn rapidus_fibo() {
  test_file("fibo.js", 55);
}