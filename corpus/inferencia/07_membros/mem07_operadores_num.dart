// R-MEM-07: operadores numéricos com as regras especiais de int/double/num.
void f<T extends int, U extends num>(int a, double b, num c, T t, U u) {
  print([
    /*@*/a + a,
    /*@*/a + b,
    /*@*/a + c,
    /*@*/b + a,
    /*@*/a / a,
    /*@*/a ~/ b,
    /*@*/a % b,
    /*@*/-a,
    /*@*/~a,
    /*@*/a.remainder(b),
    /*@*/a.clamp(1, 2),
    /*@*/a.clamp(1.5, 2),
    /*@*/t + t,
    /*@*/t + 1.5,
    /*@*/u * u,
    /*@*/t - a,
    /*@*/a * t,
  ]);
}

void main() => f<int, int>(1, 2, 3, 4, 5);
