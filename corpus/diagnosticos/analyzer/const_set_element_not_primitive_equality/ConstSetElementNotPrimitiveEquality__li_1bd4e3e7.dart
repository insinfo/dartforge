class A {
  const A();
  operator ==(other) => false;
}

main() {
  const [...[A()]];
}
