# rapidus-cilk

## A project merging [rapidus](https://github.com/maekawatoshiki/rapidus 'rapidus')(JavaScript engine) and [cilk](https://github.com/maekawatoshiki/cilk 'cilk')(LLVM-like compiler infrastructure).

### limitation
- Currently, only integer is supported.
- All variables must be declared using `let`.
- Can not refer external variables (variables declared outside of the function).
- A block scope is not yet implemented. (Function scope only)
- You can use `console.log()` to print.

### Usage

```
$cargo run rapidus/tests/fibo.js
```
