// R-LIT-03: desambiguação conjunto/mapa: pelo contexto, pelos elementos, e
// `{}` sem nada é mapa.
void main() {
  var a = /*@*/{};
  var b = /*@*/{1};
  var c = /*@*/{1: 'a'};
  Set<int> d = /*@*/{};
  Iterable<int> e = /*@*/{};
  Object f = /*@*/{};
  var g = /*@*/{...b};
  var h = /*@*/{...c};
  Map<String, num> i = /*@*/{'a': 1};
  var j = /*@*/{1: 'a', 2.5: null};
  print([a, b, c, d, e, f, g, h, i, j]);
}
