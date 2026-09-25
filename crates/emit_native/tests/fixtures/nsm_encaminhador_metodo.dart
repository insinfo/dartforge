abstract class Servico {
  String saudacao(String quem);
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

void main() {
  final p = Proxy();
  print(p.saudacao('mundo'));
  print(p.log.first);
}
