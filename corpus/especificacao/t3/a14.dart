void f(int? x) {
  switch (x) {
    case == null:
      break;
    case var y:
      String s = y;
  }
  switch (x) {
    case != null:
      break;
    case var z:
      String s = z;
  }
  switch (x) {
    case null:
      break;
    case null:
      break;
    case var w:
      String s = w;
  }
  switch (x) {
    case > 0:
      break;
    case var v:
      String s = v;
  }
  if (x case == null) {
  } else {
    String s = x;
  }
}

void g(Null n) {
  switch (n) {
    case != null:
      print(1);
    case null:
      print(2);
    case _:
      print(3);
  }
}
