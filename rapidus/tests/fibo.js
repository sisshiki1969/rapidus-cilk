function fibo(x) {
  if (x == 1) {
    return 1
  } else if (x == 2) {
    return 1
  } else {
    return fibo_sub(x - 1, x - 2)
  }
  function fibo_sub(x, y) {
    return fibo(x) + fibo(y)
  }
}

let x = fibo(10)
console.log(x)
return x
