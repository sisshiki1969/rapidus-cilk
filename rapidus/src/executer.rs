use crate::node::{BinOp, FormalParameter, Node, NodeBase};
use crate::parser;
use cilk::codegen::x64::exec::jit::{GenericValue, JITCompiler};
use cilk::ir::builder::Builder;
pub use cilk::ir::function::FunctionId;
pub use cilk::module::Module;
use cilk::{
  codegen::x64::{dag, machine},
  ir::{function, module, types},
};
pub use cilk::{
  exec::{
    interpreter::interp::{ConcreteValue, Interpreter},
    jit::x64::regalloc::RegisterAllocator,
  },
  ir::{opcode::ICmpKind, value::*},
};

use std::collections::HashMap;
use std::time::Instant;
extern crate clap;
extern crate libc;

pub fn compile_file(file_name: impl Into<String>) -> Result<Module, String> {
  let mut parser = match parser::Parser::load_module(file_name.into()) {
    Ok(ok) => ok,
    Err(err) => return Err(format!("{:?}", err)),
  };

  let node = match parser.parse_all() {
    Ok(ok) => ok,
    Err(err) => {
      parser.handle_error(&err);
      return Err(format!("{:?}", err));
    }
  };
  //println!("{:?}", node);

  let mut module = module::Module::new("cilk");
  module.add_function(function::Function::new(
    "cilk.println.i32",
    types::Type::Void,
    vec![types::Type::Int32],
  ));
  let mut func_queue: Vec<(FunctionId, Vec<FormalParameter>, Node)> = vec![];
  let main = module.add_function(function::Function::new("main", types::Type::Int32, vec![]));
  func_queue.push((main, vec![], node));

  while let Some((function_id, params, node)) = func_queue.pop() {
    let fc = FuncCompiler::new(&mut module, function_id);
    let func_map = fc.compile(&params, &node);
    for func in &func_map {
      func_queue.push(func.clone());
    }
  }

  #[cfg(debug_assertions)]
  {
    for (f_id, func) in &module.functions {
      println!("{:?} {}", f_id, func.to_string(&module));
    }
  };

  RegisterAllocator::new(&module).analyze();
  Ok(module)
}

pub fn execute_jit(m: &mut Module) -> Result<GenericValue, String> {
  let mut dag_module = dag::convert::ConvertToDAG::new(&m).convert_module();
  dag::combine::Combine::new().combine_module(&mut dag_module);
  /*
  for (_, dag_func) in &dag_module.functions {
    for id in &dag_func.dag_basic_blocks {
      let bb = &dag_func.dag_basic_block_arena[*id];
      println!("{}: {:?}", id.index(), bb);
    }
    for (id, dag) in &dag_func.dag_arena {
      println!("{}: {:?}", id.index(), dag);
    }
  }
  */

  let mut machine_module = dag::convert_machine::ConvertToMachine::new().convert_module(dag_module);
  machine::phi_elimination::PhiElimination::new().run_on_module(&mut machine_module);
  machine::two_addr::TwoAddressConverter::new().run_on_module(&mut machine_module);
  machine::regalloc::RegisterAllocator::new().run_on_module(&mut machine_module);

  /*
  let mut idx = 0;
  for (_, machine_func) in &machine_module.functions {
    for bb_id in &machine_func.basic_blocks {
      let bb = &machine_func.basic_block_arena[*bb_id];
      println!("Machine basic block: {:?}", bb);
      for instr in &*bb.iseq_ref() {
        println!("{}: {:?}", idx, machine_func.instr_arena[*instr]);
        idx += 1;
      }
      println!()
    }
  }
  */

  let mut jit = JITCompiler::new(&machine_module);
  jit.compile_module();
  let func = machine_module.find_function_by_name("main").unwrap();
  let now = Instant::now();
  let ret = jit.run(func, vec![GenericValue::Int32(0)]);
  println!("duration: {:?}", Instant::now().duration_since(now));
  Ok(ret)
}

pub fn execute_interpreter(module: &mut Module) -> Result<ConcreteValue, String> {
  let main = module.find_function_by_name("main").unwrap();
  let mut interp = Interpreter::new(&module);
  let ret = interp.run_function(main, vec![ConcreteValue::Int32(9)]);
  Ok(ret)
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

  pub fn collect_var_decl(&mut self, node: &'a crate::node::Node) {
    match &node.base {
      NodeBase::StatementList(nodes) => {
        for node in nodes {
          self.collect_var_decl(&node);
        }
      }
      NodeBase::Block(nodes) => {
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
      NodeBase::If(_, then_, else_) => {
        self.collect_var_decl(&then_);
        self.collect_var_decl(&else_);
      }
      NodeBase::While(_, body) => {
        self.collect_var_decl(&body);
      }
      NodeBase::For(init, _, step, body) => {
        self.collect_var_decl(&init);
        self.collect_var_decl(&step);
        self.collect_var_decl(&body);
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
            .insert(name.clone(), (func_id, params.clone(), *body.clone()));
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
        BinOp::Le => {
          let lhs_v = self.visit(lhs);
          let rhs_v = self.visit(rhs);
          self.builder.build_icmp(ICmpKind::Le, lhs_v, rhs_v)
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

        Value::None
      }
      NodeBase::While(cond, body) => {
        let cond_bb = self.builder.append_basic_block();
        let body_bb = self.builder.append_basic_block();
        let cont_bb = self.builder.append_basic_block();
        self.builder.build_br(cond_bb);
        self.builder.set_insert_point(cond_bb);
        let cond_v = self.visit(cond);
        self.builder.build_cond_br(cond_v, body_bb, cont_bb);
        self.builder.set_insert_point(body_bb);
        self.visit(body);
        self.builder.build_br(cond_bb);
        self.builder.set_insert_point(cont_bb);

        Value::None
      }
      NodeBase::For(init, cond, step, body) => {
        let init_bb = self.builder.append_basic_block();
        let cond_bb = self.builder.append_basic_block();
        let body_bb = self.builder.append_basic_block();
        let cont_bb = self.builder.append_basic_block();
        self.builder.build_br(init_bb);
        self.builder.set_insert_point(init_bb);
        self.visit(init);
        self.builder.build_br(cond_bb);
        self.builder.set_insert_point(cond_bb);
        let cond_v = self.visit(cond);
        self.builder.build_cond_br(cond_v, body_bb, cont_bb);
        self.builder.set_insert_point(body_bb);
        self.visit(body);
        self.visit(step);
        self.builder.build_br(cond_bb);
        self.builder.set_insert_point(cont_bb);

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
          NodeBase::Identifier(name) => self.find_func_name(name),
          NodeBase::Member(parent, member) => {
            if parent.base == NodeBase::Identifier("console".to_string())
              && *member == "log".to_string()
            {
              self
                .builder
                .module
                .find_function_by_name("cilk.println.i32")
                .unwrap()
            } else {
              panic!("Member expression is not implemented yet.");
            }
          }
          _ => unimplemented!("callee should be Identifier."),
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
      NodeBase::Nope => Value::None,
      _ => unimplemented!("{:?}", node.base),
    }
  }

  fn find_func_name(&mut self, name: &String) -> FunctionId {
    match self.function_map.get(name) {
      Some(v) => return v.0,
      None => {}
    };
    let function_name = self.function_name.clone();
    let mut func_name_vec: Vec<&str> = function_name.split('.').collect();
    while let Some(fname) = func_name_vec.pop() {
      if *name == *fname {
        let fullname = format!("{}.{}", func_name_vec.join("."), fname);
        return self
          .builder
          .module
          .find_function_by_name(fullname.as_str())
          .unwrap();
      }
    }
    panic!("function not found: {}", name);
  }

  fn get_variable(&mut self, name: &String) -> Value {
    match self.variable_map.get(name) {
      Some(v) => *v,
      None => panic!("Undefined var: {:?}", name),
    }
  }
}
