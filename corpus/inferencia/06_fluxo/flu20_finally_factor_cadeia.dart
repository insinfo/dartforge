// R-FLU-20: `finally` promove para depois do try; o ramo falso de `is` usa
// factor(T, S); a promoção de `?.` não sai da cadeia.
import 'dart:async';

void f(Object o, int? x, FutureOr<int> fo, int? y) {
  try {
    print(0);
  } finally {
    o as int;
  }
  print(/*@*/o);
  if (x is int) {
    print(/*@*/x);
  } else {
    print(/*@*/x);
  }
  if (fo is int) {
  } else {
    print(/*@*/fo);
  }
  y?.isEven;
  print(/*@*/y);
}

void main() => f(1, 2, 3, 4);