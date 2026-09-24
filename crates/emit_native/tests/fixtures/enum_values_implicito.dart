enum E {
  a,
  b;

  E get proximo => values[(index + 1) % values.length];
  static E primeiro() => values.first;
}

void main() {
  print(E.a.proximo);
  print(E.b.proximo);
  print(E.primeiro());
  print(E.values.length);
}
