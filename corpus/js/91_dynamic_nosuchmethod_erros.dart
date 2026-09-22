// Chamadas dinâmicas inválidas capturadas como NoSuchMethodError: método/getter inexistente, não-função, aridade, null.
class Coisa {
  int valor = 1;
  int get somenteLeitura => 2;
  int metodo(int a) => a + valor;
  String nomeado({required int x}) => 'x=$x';
}

void tenta(String rotulo, void Function() acao) {
  try {
    acao();
    print('$rotulo: nao lancou');
  } on NoSuchMethodError catch (e) {
    print('$rotulo: NoSuchMethodError');
    print(e.toString().isNotEmpty);
  } catch (e) {
    print('$rotulo: outro erro ${e.runtimeType == NoSuchMethodError}');
  }
}

void main() {
  dynamic c = Coisa();
  tenta('metodo existente', () => print(c.metodo(1)));
  tenta('metodo inexistente', () => c.inexistente());
  tenta('metodo inexistente com args', () => c.inexistente(1, 2, k: 3));
  tenta('getter inexistente', () => print(c.campoInexistente));
  tenta('setter inexistente', () {
    c.campoInexistente = 5;
  });
  tenta('setter em getter-only', () {
    c.somenteLeitura = 5;
  });
  tenta('chamar campo int', () => c.valor());
  tenta('chamar getter int', () => c.somenteLeitura(1));
  tenta('aridade a mais', () => c.metodo(1, 2));
  tenta('aridade a menos', () => c.metodo());
  tenta('nomeado inexistente', () => c.metodo(1, extra: 2));
  tenta('nomeado faltando', () => c.nomeado());
  tenta('nomeado errado', () => c.nomeado(y: 1));
  tenta('nomeado certo', () => print(c.nomeado(x: 7)));

  dynamic nulo;
  tenta('null.foo', () => nulo.foo);
  tenta('null.foo()', () => nulo.foo());
  tenta('null.length', () => print(nulo.length));
  tenta('null[0]', () => nulo[0]);
  tenta('null + 1', () => nulo + 1);
  tenta('null.toString()', () => print(nulo.toString()));
  tenta('null.hashCode', () => print(nulo.hashCode == null.hashCode));
  tenta('null == null', () => print(nulo == null));
  tenta('null()', () => nulo());

  dynamic s = 'abc';
  tenta('string.foo', () => s.foo);
  tenta('string.add', () => s.add('d'));
  tenta('string[0]', () => print(s[0]));
  tenta('string()', () => s());
  tenta('string.length = 1', () {
    s.length = 1;
  });

  dynamic n = 42;
  tenta('int.length', () => n.length);
  tenta('int.foo()', () => n.foo());
  tenta('int()', () => n());
  tenta('int.abs()', () => print(n.abs()));

  dynamic l = [1, 2];
  tenta('list.foo', () => l.foo);
  tenta('list.add(3)', () => l.add(3));
  tenta('list()', () => l());
  tenta('list.length', () => print(l.length));

  dynamic f = (int a) => a * 2;
  tenta('f(2)', () => print(f(2)));
  tenta('f()', () => f());
  tenta('f(1, 2)', () => f(1, 2));
  tenta('f(a: 1)', () => f(a: 1));
  tenta('f.foo', () => f.foo);
  tenta('f.call(3)', () => print(f.call(3)));

  dynamic m = {'k': 1};
  tenta('map.k', () => m.k);
  tenta("map['k']", () => print(m['k']));
  tenta('map.foo()', () => m.foo());
}
