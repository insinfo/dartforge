// `import '' as self;` — a URI vazia é a própria biblioteca (o
// `native.dart` gerado pelo ffigen do `sqlite3`). O carregador juntava ''
// ao diretório e tentava ler o diretório (docs/NATIVO-PROJETOS-REAIS.md, C15).

import '' as self;

int contador = 3;

int dobro(int x) => x * 2;

class Caixa {
  final int v;
  Caixa(this.v);
  @override
  String toString() => 'Caixa($v)';
}

void main() {
  self.contador += 4;
  print(self.contador);
  print(self.dobro(self.contador));
  print(self.Caixa(9));
  print(identical(self.dobro, dobro));
}
