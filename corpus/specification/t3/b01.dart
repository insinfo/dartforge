void f(int? n, num? m) {
  switch (n) {
    case int _:
      break;
    case var z:
      String s = z;
  }
  switch (n) {
    case int _:
      break;
    case 1:
      break;
  }
  if (n case int _) {
  } else {
    String s = n;
  }
  if (n is int) {
  } else {
    String s = n;
  }
  switch (m) {
    case int _:
      break;
    case var z:
      String s = z;
  }
  switch (m) {
    case num _:
      break;
    case var z:
      String s = z;
  }
}
