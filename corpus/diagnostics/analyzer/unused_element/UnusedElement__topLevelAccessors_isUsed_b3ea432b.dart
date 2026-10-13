int? get _c => 1;
void set _c(int? x) {}
int f() {
  return _c ??= 7;
}
