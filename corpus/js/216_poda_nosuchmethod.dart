// Poda do perfil de produção (docs/JS-PRODUCAO.md §1.7): `noSuchMethod`.
// O nsm do usuário vive em toda classe instanciada (o runtime o chama por
// nome), e os encaminhadores dos membros abstratos vivem pelo seletor. Se a
// poda errar, o caso imprime "caso <nome>: ERRO".

void caso(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('caso $nome: ERRO $e');
  }
}

abstract class Servico {
  String saudacao(String quem);
  int get versao;
  set modo(String m);
  String naoChamado();
}

class Proxy implements Servico {
  final List<String> log = [];

  @override
  dynamic noSuchMethod(Invocation i) {
    final nome = i.memberName.toString();
    log.add(nome);
    if (i.isGetter) return 42;
    if (i.isSetter) return null;
    return 'nsm(${i.positionalArguments.join(',')})';
  }
}

class Gravador {
  final Map<Symbol, int> contagem = {};

  @override
  dynamic noSuchMethod(Invocation i) {
    contagem[i.memberName] = (contagem[i.memberName] ?? 0) + 1;
    return contagem.length;
  }
}

void main() {
  final p = Proxy();
  caso('encaminhador de método', () => p.saudacao('mundo'));
  caso('encaminhador de getter', () => p.versao);
  caso('encaminhador de setter', () {
    p.modo = 'rápido';
    return p.log.length;
  });
  caso('dinâmico cai no nsm', () {
    dynamic d = Gravador();
    d.qualquerCoisa(1);
    d.outraCoisa;
    return d.maisUma(2, 3);
  });
  caso('symbol do nome', () => p.log.first);
}
