void f((int, String) r, (Object, Object?) q) {
  switch (r) {
    case (int a, String b):
      print(a);
    case (_, _):
      print(2);
  }
  if (q case (int a, _)) {
    q.$1.isEven;
    String s = q;
  }
  switch (q) {
    case (int _, String _):
      break;
    case (var a, var b):
      String s = a;
  }
}
