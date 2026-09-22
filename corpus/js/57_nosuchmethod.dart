// noSuchMethod: métodos/getters/setters inexistentes via dynamic, Invocation, interface só com noSuchMethod.
class Eco {
  final Map<Symbol, Object?> _campos = {};

  @override
  dynamic noSuchMethod(Invocation inv) {
    if (inv.isGetter) {
      print('getter ${inv.memberName == #nome ? "nome" : "outro"}');
      return _campos[inv.memberName] ?? 'sem valor';
    }
    if (inv.isSetter) {
      print('setter recebeu ${inv.positionalArguments}');
      _campos[inv.memberName] = inv.positionalArguments.first;
      return null;
    }
    if (inv.isMethod) {
      print('metodo: pos=${inv.positionalArguments} '
          'nomeados=${inv.namedArguments.length} '
          'tipos=${inv.typeArguments.length}');
      if (inv.memberName == #soma) {
        var total = 0;
        for (final a in inv.positionalArguments) {
          total += a as int;
        }
        return total;
      }
      if (inv.memberName == #repete) {
        final vezes = inv.namedArguments[#vezes] as int? ?? 1;
        return List.filled(vezes, inv.positionalArguments.first).join();
      }
      return 'metodo generico';
    }
    return super.noSuchMethod(inv);
  }
}

abstract class Calculadora {
  int dobra(int x);
  int triplica(int x);
  String get nome;
}

class CalculadoraProxy implements Calculadora {
  final List<String> log = [];

  @override
  dynamic noSuchMethod(Invocation inv) {
    if (inv.memberName == #dobra) {
      log.add('dobra');
      return (inv.positionalArguments[0] as int) * 2;
    }
    if (inv.memberName == #triplica) {
      log.add('triplica');
      return (inv.positionalArguments[0] as int) * 3;
    }
    if (inv.memberName == #nome) {
      log.add('nome');
      return 'proxy';
    }
    return super.noSuchMethod(inv);
  }
}

class Parcial {
  int existente() => 1;
  @override
  dynamic noSuchMethod(Invocation inv) {
    print('caiu em noSuchMethod: isMethod=${inv.isMethod} '
        'isGetter=${inv.isGetter} isSetter=${inv.isSetter} '
        'isAccessor=${inv.isAccessor}');
    return -1;
  }
}

void main() {
  final dynamic e = Eco();
  print(e.nome);
  e.nome = 'eco';
  print(e.nome);
  print(e.idade);
  print(e.soma(1, 2, 3));
  print(e.repete('ab', vezes: 3));
  print(e.repete('x'));
  print(e.qualquer());
  print(e.generico<int>(5));
  print(e.toString().length > 0);
  print(e.hashCode == e.hashCode);

  final Calculadora c = CalculadoraProxy();
  print(c.dobra(4));
  print(c.triplica(5));
  print(c.nome);
  print(c.dobra(1) + c.triplica(1));
  print((c as CalculadoraProxy).log);
  print(c is Calculadora);

  final dynamic p = Parcial();
  print(p.existente());
  print(p.inexistente());
  print(p.inexistente(1, a: 2));
  print(p.campo);
  p.campo = 3;
  print(p == p);
  print(p.runtimeType);

  print(#foo == #foo);
  print(#foo == #bar);
  print(const Symbol('foo') == #foo);
}
