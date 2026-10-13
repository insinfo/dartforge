void f(bool? b, int x) {
  switch (b) {
    case null: break;
    case true: break;
    case null: break;
    case false: break;
  }
  var r = switch (b) {
    null => 0,
    Null _ => 1,
    null => 2,
    _ => 3,
  };
  switch (b) {
    case true: break;
    case bool _: break;
    case null: break;
    case 1: break;
  }
  switch (x) {
    case int _ when x > 0: break;
    case 'a': break;
    case == 'a': break;
    case > 'a': break;
  }
  if (b case true || null) {}
  if (b case != null && null) {}
  if (b case == null) {} else if (b case null) {}
  print(r);
}
