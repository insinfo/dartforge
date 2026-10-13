class Base {
  Base();
  const factory Base.empty() = _Empty;
  late final int v;
}

class _Empty implements Base {
  const _Empty();
  int get v => 0;
  set v(_) {}
}
