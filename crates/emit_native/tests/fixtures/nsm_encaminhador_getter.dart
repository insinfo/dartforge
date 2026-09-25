abstract class Servico {
  int get versao;
}

class Proxy implements Servico {
  @override
  dynamic noSuchMethod(Invocation i) {
    if (i.isGetter) return 42;
    if (i.isSetter) return null;
    return 'nsm(${i.positionalArguments.join(',')})';
  }
}

void main() {
  final p = Proxy();
  print(p.versao);
}
