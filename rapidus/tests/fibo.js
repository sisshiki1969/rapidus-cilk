function fibo(x) {
  if (x <= 2) {
    return 1
  } else {
    return fibo(x - 1) + fibo(x - 2)
  }
}

let x = fibo(35)
console.log(x)
return x
