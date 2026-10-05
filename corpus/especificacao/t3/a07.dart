void f(int x, int? n, num m) {
  if (x case var a?) {}
  if (x case var b!) {}
  if (x case _ as int) {}
  if (x case _ as num) {}
  if (n case var c? && var d?) {}
  if (n case var e! && var g!) {}
  if (m case int _ as int) {}
  if (n case _ as int? && _ as int) {}
  var (h!) = x;
  var (i as int) = x;
}
