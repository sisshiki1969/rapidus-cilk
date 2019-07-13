extern crate cilk;
use cilk::ir::builder::Builder;
use cilk::ir::function::FunctionId;
use cilk::module::Module;
use cilk::{
  exec::{interpreter::interp, jit::x64::compiler, jit::x64::regalloc},
  ir::{function, module, types, value::*, opcode::ICmpKind},
};
use rapidus::node::{BinOp, FormalParameter, Node, NodeBase};
use rapidus::parser;
use std::collections::HashMap;
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

  let mut parser = match parser::Parser::load_module(file_name.clone()) {
    Ok(ok) => ok,
    Err(_) => return,
  };

  let node = match parser.parse_all() {
    Ok(ok) => ok,
    Err(err) => {
      parser.handle_error(&err);
      return;
    }
  };
  //println!("{:?}", node);

  let mut module = module::Module::new("cilk");
  let mut func_queue: Vec<(FunctionId, Vec<FormalParameter>, Node)> = vec![];
  let main = module.add_function(function::Function::new(
    "main",
    types::Type::Int32,
    vec![],
  ));
  func_queue.push((main, vec![], node));

  while let Some((function_id, params, node)) = func_queue.pop() {
    let fc = FuncCompiler::new(&mut module, function_id);
    let func_map = fc.compile(&params, &node);
    for func in &func_map {
      func_queue.push(func.clone());
    }
  }

  for (f_id, func) in &module.functions {
    println!("{:?} {}", f_id, func.to_string(&module));
  }

  let mut interp = interp::Interpreter::new(&module);
  let ret = interp.run_function(main, vec![interp::ConcreteValue::Int32(9)]);
  println!("exec: {:?}", ret);

  regalloc::RegisterAllocator::new(&module).analyze();

  let mut jit = compiler::JITCompiler::new(&module);
  jit.compile_module();

  let ret = jit.run(main, vec![]);
  println!("jit: {:?}", ret);

}

#[derive(Debug)]
pub struct FuncCompiler<'a> {
  function_id: FunctionId,
  function_name: String,
  builder: Builder<'a>,
  variable_map: HashMap<String, Value>,
  arguments_map: HashMap<String, usize>,
  function_map: HashMap<String, (FunctionId, Vec<FormalParameter>, Node)>,
}

impl<'a> FuncCompiler<'a> {
  pub fn new(module: &'a mut module::Module, function_id: FunctionId) -> Self {
    let function_name = module.function_ref(function_id).name.clone();
    let builder = Builder::new(module, function_id);
    FuncCompiler {
      function_id,
      function_name,
      builder,
      variable_map: HashMap::default(),
      arguments_map: HashMap::default(),
      function_map: HashMap::default(),
    }
  }

  pub fn get_module(self) -> &'a mut Module {
    self.builder.module
  }

  pub fn compile(
    mut self,
    params: &Vec<FormalParameter>,
    node: &'a Node,
  ) -> Vec<(FunctionId, Vec<FormalParameter>, Node)> {
    self.set_arguments(params);
    let entry = self.builder.append_basic_block();
    self.builder.set_insert_point(entry);
    self.collect_var_decl(node);
    let _v = self.visit(node);
    self
      .builder
      .build_ret(Value::Immediate(ImmediateValue::Int32(0)));
    self.function_map.into_iter().map(|x| x.1).collect()
  }

  pub fn set_arguments(&mut self, params: &Vec<FormalParameter>) {
    for (i, param) in params.iter().enumerate() {
      self.arguments_map.insert(param.name.clone(), i);
    }
  }

  pub fn collect_var_decl(&mut self, node: &'a rapidus::node::Node) {
    match &node.base {
      NodeBase::StatementList(nodes) => {
        for node in nodes {
          self.collect_var_decl(&node);
        }
      }
      NodeBase::VarDecl(name, _init, _kind) => {
        if self.variable_map.contains_key(name) {
          panic!("duplicated declaration of variable: named {:?}", name);
        } else {
          let v = self.builder.build_alloca(types::Type::Int32);
          self.variable_map.insert(name.clone(), v);
        }
      }
      NodeBase::FunctionDecl(name, params, body) => {
        if self.function_map.contains_key(name) {
          panic!("duplicated declaration of function: named {:?}", name);
        } else {
          let decl_function_name = format!("{}.{}", self.function_name, name);
          let func_id = self.builder.module.add_function(function::Function::new(
            decl_function_name.as_str(),
            types::Type::Int32,
            vec![types::Type::Int32; params.len()],
          ));
          self
            .function_map
            .insert(decl_function_name.clone(), (func_id, params.clone(), *body.clone()));
        }
      }
      _ => {}
    }
  }

  pub fn visit(&mut self, node: &Node) -> Value {
    match &node.base {
      NodeBase::StatementList(nodes) => {
        let mut v = Value::None;
        for node in nodes {
          v = self.visit(node);
        }
        v
      }
      NodeBase::Block(nodes) => {
        let mut v = Value::None;
        for node in nodes {
          v = self.visit(node);
        }
        v
      }
      NodeBase::BinaryOp(lhs, rhs, op) => match op {
        BinOp::Add => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_add(lhs_v, rhs_v)
        }
        BinOp::Sub => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_sub(lhs_v, rhs_v)
        }
        BinOp::Mul => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_mul(lhs_v, rhs_v)
        }
        BinOp::Rem => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_rem(lhs_v, rhs_v)
        }
        BinOp::Eq => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_icmp(ICmpKind::Eq, lhs_v, rhs_v)
        }
        _ => unimplemented!("{:?}", op),
      },
      NodeBase::Assign(lhs, rhs) => match &lhs.base {
        NodeBase::Identifier(name) => {
          let rhs_v = self.visit(rhs);
          let val_v = self.get_variable(name);
          self.builder.build_store(rhs_v, val_v);
          rhs_v
        }
        _ => unimplemented!(
          "Left hand side of assignment statement should be an identifier. {:?}",
          lhs
        ),
      },
      NodeBase::If(cond, then_, else_) => {
        let cond_v = self.visit(cond);
        let then_bb = self.builder.append_basic_block();
        let else_bb = self.builder.append_basic_block();
        let cont_bb = self.builder.append_basic_block();
        self.builder.build_cond_br(cond_v, then_bb, else_bb);
        self.builder.set_insert_point(then_bb);
        self.visit(then_);
        self.builder.build_br(cont_bb);
        self.builder.set_insert_point(else_bb);
        self.visit(else_);
        self.builder.build_br(cont_bb);
        self.builder.set_insert_point(cont_bb);
        /*
        let ret = builder.build_phi(vec![
        (
            value::Value::Immediate(value::ImmediateValue::Int32(1)),
            then_bb,
        ),
        (val3, else_bb),
    ]);
    */
        Value::None
      }
      NodeBase::VarDecl(name, init, _kind) => {
        let init_v = match init {
          Some(init) => self.visit(init),
          None => Value::Immediate(ImmediateValue::Int32(0)),
        };
        let v = self.get_variable(name);
        self.builder.build_store(init_v, v) // returns Value::None
      }
      NodeBase::FunctionDecl(_name, _params, _body) => Value::None,
      NodeBase::Call(callee, args) => {
        let callee_id = match &callee.base {
          NodeBase::Identifier(name) => {
            if *name == self.function_name.split('.').last().unwrap() {
              self.function_id
            } else {
              let name = format!("{}.{}", self.function_name, *name);
              match self.function_map.get(&name) {
                Some(v) => v.0,
                None => {
                  println!("{:?}", self.function_map);
                  panic!("function not found: {}", name);
                }
              }
            }
          }
          _ => unimplemented!("callee should be Identifier(str)"),
        };
        let mut args_v = vec![];
        for arg in args {
          let v = self.visit(arg);
          args_v.push(v);
        }
        self.builder.build_call(Value::Function(callee_id), args_v)
      }
      NodeBase::Return(ret) => {
        let ret_v = match ret {
          Some(node) => self.visit(node),
          None => Value::None,
        };
        self.builder.build_ret(ret_v)
      }
      NodeBase::Identifier(name) => {
        match self.arguments_map.get(name) {
          Some(v) => return self.builder.get_param(*v).unwrap(),
          None => {}
        };
        let v = self.get_variable(name);
        self.builder.build_load(v)
      }
      NodeBase::Number(x) => {
        let x_i32 = *x as i32;
        Value::Immediate(ImmediateValue::Int32(x_i32))
      }
      _ => unimplemented!("{:?}", node.base),
    }
  }

  fn get_variable(&mut self, name: &String) -> Value {
    match self.variable_map.get(name) {
      Some(v) => *v,
      None => panic!("Undefined var: {:?}", name),
    }
  }
}
