enum _E {
  v._foo();
  const _E._foo() : this._bar();
  const _E._bar();
}

void f() {
  _E.v;
}
