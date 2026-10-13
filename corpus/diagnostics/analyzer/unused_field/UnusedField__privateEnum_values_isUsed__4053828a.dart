enum _E {
  v;

  static _E fromIndex(int index) {
    return values[index];
  }
}

void f() {
  _E.fromIndex(0);
}
