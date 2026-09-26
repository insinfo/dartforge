// Sets constantes de records, tear-off estático de extensão, hash de
// closures, WeakReference e Expando (projetos reais: html, collection).
const ns = 'http://www.w3.org/1999/xhtml';
const escopo = {(ns, 'applet'), (ns, 'caption'), (ns, 'html'), (ns, 'table'), (ns, 'td')};
(String, String) tupla(String n) => (ns, n);

extension Minusc on String {
  String minusc() => codeUnits.any(_maiuscula) ? String.fromCharCodes(codeUnits.map(_paraMin)) : this;
  static bool _maiuscula(int c) => c >= 65 && c <= 90;
  static int _paraMin(int c) => _maiuscula(c) ? c + 32 : c;
}

class C {
  final int v;
  C(this.v);
  int dobro() => v * 2;
}

int f(int x) => x;
final nomes = Expando<String>('nomes');

void main() {
  print(escopo.contains(tupla('html')));
  print(escopo.contains(tupla('div')));
  print(const {'a', 'b', 'c', 'd', 'e'}.contains('d'));
  print('ABc'.minusc());
  final g = (int x) => x + 1;
  final c = C(4);
  print({g, g, f, f, c.dobro, c.dobro}.length);
  print(c.dobro.hashCode == c.dobro.hashCode);
  final w = WeakReference(c);
  print(w.target?.v);
  nomes[c] = 'quatro';
  print(nomes[c]);
  print(nomes[C(5)]);
}
