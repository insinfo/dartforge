void f(int x, int y, String s, int? n, double d, num m) {
  switch (s) {
    case 1: break;
    case 'a': break;
    case null: break;
  }
  switch (n) {
    case 1: break;
    case null: break;
    case null: break;
    case 'a': break;
  }
  switch (d) {
    case 1: break;
    case 1.5: break;
    case 'a': break;
  }
  switch (x) {
    case 1.5: break;
    case 2: break;
  }
  switch (m) {
    case 1: break;
    case 1.5: break;
    case true: break;
  }
}
