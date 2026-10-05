void f(Null n, int? m) {
  if (n case var a?) {
    print(a);
  } else {
    print(0);
  }
  if (n case _!) {
    print(1);
  }
  if (n case _ as int) {
    print(2);
  }
  if (m case null) {
    print(3);
  } else {
    m.isEven;
  }
}
