void f(Object x, int i) {
  if (x case _ && var a) {}
  if (x case int _ && _) {}
  if (i case int _) {}
  if (i case _) {}
  switch (i) {
    case int _ && < 3:
      break;
    case _ && _:
      break;
  }
  var (int _, b) = (i, i);
  if (x case (_, int _)) {}
  if (x case [_, int _ && _]) {}
}
