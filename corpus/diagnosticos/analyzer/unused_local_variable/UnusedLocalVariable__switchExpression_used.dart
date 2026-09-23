Object? f(Object? x) {
  return switch (x) {
    (int a,) => a,
    _ => 0,
  };
}
