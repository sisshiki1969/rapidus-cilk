function fibo(x) {
  if (x == 1) {
    return 1
  } else {
    if (x == 2) {
      return 1
    } else {
      return fibo(x - 1) + fibo(x - 2)
    }
  }
}

return fibo(5)
