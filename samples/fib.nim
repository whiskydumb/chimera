## iterative fibonacci.

proc fib(n: Natural): uint64 =
  var
    a: uint64 = 0
    b: uint64 = 1
  for _ in 0 ..< n:
    (a, b) = (b, a + b)
  a

when isMainModule:
  for i in 0 .. 10:
    echo fib(i)
