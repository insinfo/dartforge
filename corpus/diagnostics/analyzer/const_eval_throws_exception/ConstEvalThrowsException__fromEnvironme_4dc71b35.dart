const b = bool.fromEnvironment('foo');

main() {
  const l1 = [1, 2, 3];
  const l2 = [if (b) ...l1];
  print(l2);
}
