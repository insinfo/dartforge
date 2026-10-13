typedef F<T> = void Function(T);
f() {
  try {
  } on F catch (e) {
    e;
  }
}
