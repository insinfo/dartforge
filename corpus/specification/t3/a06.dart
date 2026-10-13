void f(num? x, Object? o) {
  if (x case int _ && var a) {
    String s = a;
  }
  if (x case != null && var b) {
    String s = b;
  }
  if (x case var c?) {
    String s = c;
    String t = x;
  }
  if (o case num _ && int? d) {
    String s = d;
  }
  if (x case int? e?) {
    String s = e;
  }
}
