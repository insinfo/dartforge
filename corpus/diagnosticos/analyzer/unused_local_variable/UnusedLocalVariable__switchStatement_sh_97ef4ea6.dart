void f(Object? x) {
  switch (x) {
    case (var a,):
    case [var a,]:
      a;
  };
}
