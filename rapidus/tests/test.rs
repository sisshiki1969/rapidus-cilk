extern crate rapidus;
use rapidus::executer;
use rapidus::executer::{ConcreteValue, GenericValue};

fn test_file(file_name: &str, expected: i32) {
  let mut m = match executer::compile_file(format!("tests/{}", file_name)) {
    Ok(m) => m,
    Err(e) => panic!("Failed to construct module. {}", e),
  };
  let res = match executer::execute_interpreter(&mut m) {
    Ok(v) => v,
    Err(e) => panic!("Failed in interpreter. {}", e),
  };
  if res != ConcreteValue::Int32(expected) {
    panic!("expected Int32({}), but {:?}", expected, res);
  }
  let res = match executer::execute_jit(&mut m) {
    Ok(v) => v,
    Err(e) => panic!("Failed to jit compile. {}", e),
  };
  if res != GenericValue::Int32(expected) {
    panic!("expected Int32({}), but {:?}", expected, res);
  }
}

#[test]
fn rapidus_fibo() {
  test_file("fibo.js", 55);
}

#[test]
fn rapidus_while() {
  test_file("while.js", 9);
}

#[test]
fn rapidus_prime() {
  test_file("prime.js", 19);
}
