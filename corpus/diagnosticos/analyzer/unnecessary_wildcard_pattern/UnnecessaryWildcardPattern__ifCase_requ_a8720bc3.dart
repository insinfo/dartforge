void f(Object? x) {
  if (x case {'foo': _, 'bar': 0}) {}
}
