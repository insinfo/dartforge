// Imports com prefixo: funções/classes/enums/constantes homônimas em a e b, variável top-level de outra lib modificada via prefixo, prefixo em tipos e is/as.
import 'a.dart' as a;
import 'b.dart' as b;
import 'b.dart' as b2;

String nome() => 'main';

a.Forma criaA() => a.Forma('círculo');

void recebe(b.Forma f) => print('recebeu ${f.descreve()}');

void main() {
  print(nome());
  print(a.nome());
  print(b.nome());
  print(b2.nome());
  print('--');
  final fa = criaA();
  final fb = b.Forma(4);
  print(fa.descreve());
  print(fb.descreve());
  recebe(fb);
  print(fa is a.Forma);
  print(fb is b.Forma);
  final Object o = fb;
  print(o is a.Forma);
  print((o as b.Forma).lados);
  print(fa.runtimeType == a.Forma);
  print(fb.runtimeType == b.Forma);
  print(a.Forma == b.Forma);
  print(b.Forma == b2.Forma);
  print('--');
  print(a.Nivel.alto);
  print(a.Nivel.values.length);
  print(b.Nivel.values.length);
  print(a.Nivel.medio.index);
  print(b.Nivel.dois.name);
  final a.Nivel n = a.Nivel.baixo;
  switch (n) {
    case a.Nivel.baixo:
      print('baixo');
    case a.Nivel.medio:
    case a.Nivel.alto:
      print('outro');
  }
  print('--');
  print(a.versao);
  print(b.versao);
  print(a.contador);
  print(b.contador);
  a.contador = 10;
  a.contador += 5;
  print(a.contador);
  print(a.usaContador());
  print(a.incrementa());
  print(a.contador);
  b.contador++;
  print(b.contador);
  print(b2.contador);
  b2.contador = 7;
  print(b.contador);
  print('--');
  print(b.aplica(b.maiuscula, 'abc'));
  print(b.aplica((s) => '<$s>', 'x'));
  final b.Transforma t = (s) => s * 2;
  print(t('ab'));
  print(b.Util.dobra(21));
  print(b.Util.pi);
  final lista = <a.Forma>[a.Forma('x'), a.Forma('y')];
  print(lista.map((f) => f.tipo).join(','));
  final mapa = <String, b.Forma>{'q': b.Forma(4), 't': b.Forma(3)};
  print(mapa.values.map((f) => f.lados).toList());
  final fn = a.nome;
  print(fn());
  print([a.nome, b.nome].map((f) => f()).toList());
  print('fim');
}
