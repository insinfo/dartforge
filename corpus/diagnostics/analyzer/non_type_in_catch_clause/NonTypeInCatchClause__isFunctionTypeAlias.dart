typedef F();
f() {
  try {
  } on F catch (e) {
    e;
  }
}
