/* eslint-disable no-console */

function prime(n) {
  if (n % 2 == 0) return 0
  for (let k = 3; k * k <= n; k += 2) { if (n % k == 0) return 0 }
  return 1
}

let max = 2

for (let i = 2; i <= 20; i += 1) {
  if (prime(i) == 1) {
    console.log(i)
    max = i
  }
}

return max