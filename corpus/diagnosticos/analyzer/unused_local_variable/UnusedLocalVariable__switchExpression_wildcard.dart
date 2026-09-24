Object? f(Object? x) {
  return switch (x) {
    (int _,) => 0,
    _ => 0,
  };
}
