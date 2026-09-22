// Privacidade por biblioteca: main só vê a API pública de lib.dart; _Impl privada chega como Servico; _campo só via getter; nomes privados do main são independentes.
import 'lib.dart';

// Um _foo no main não conflita com o _foo de lib.dart: são nomes distintos por biblioteca.
int _foo(int x) => x + 1000;

class _Impl {
  @override
  String toString() => '_Impl do main';
}

void main() {
  print(usaFoo(2));
  print(usaFoo(3));
  print(_foo(2));
  print('--');
  final s = criaServico(5);
  print(s.nome());
  print(s.executa(4));
  print(s is Servico);
  final n = criaServico(-3);
  print(n.nome());
  print(n.executa(4));
  print(_Impl());
  // s._fator não é acessível aqui: _fator é privado de lib.dart.
  print('--');
  final p = Pessoa('Ana', 30);
  print(p.nome);
  print(p.idade);
  p.envelhece();
  print(p.idade);
  p.marca('x');
  p.marca('y');
  print(p.quantasTags);
  print(p);
  final q = Pessoa.padrao();
  print(q);
  print(p.maisVelhaQue(q));
  print(q.maisVelhaQue(p));
  print(p.resumo);
  // p._nome, p._idade, Pessoa._anonima() não são visíveis aqui.
  print('--');
  final a = Auditor();
  a.trabalha();
  a.registra('c');
  print(a.historico);
  final m = Motor();
  print(m.estado);
  m.acelera();
  print(m.estado);
  print(soma(1, 2));
  print(soma(1));
  print(aplicaDobra(21));
  final Object o = criaServico(2);
  print(o is Servico);
  print((o as Servico).executa(10));
  print('fim');
}
