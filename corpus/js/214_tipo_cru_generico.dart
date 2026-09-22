// Tipo **cru**: uma classe genérica escrita sem argumentos (`Caixa`) vale
// `Caixa<dynamic>`, e a receita rti tem de trazer um argumento por parâmetro
// declarado — `raw|Caixa<@>`, nunca `raw|Caixa`. O `rti` confere a aridade em
// `_areArgumentsSubtypes` (`assert(length == _Utils.arrayLength(tArgs))`), e
// uma receita crua derruba o primeiro `is`/`as` que a atravesse.
//
// O caso que trouxe isto: `Validators.compose` do ngforms chama
// `_removeNullValidators<T>(List<T?>)` com `T` inferido como
// `ValidatorFn = Map<String, dynamic>? Function(AbstractControl)` — o
// `AbstractControl<T>` ali é cru —, e o `result.add(validator)` de dentro faz
// um `as T` contra essa receita.
abstract class Caixa<T> {
  T get valor;
}

class CaixaInt implements Caixa<int> {
  @override
  int get valor => 7;
}

// Supertipo cru: `implements Caixa` é `implements Caixa<dynamic>`, e o
// `addRules` da subclasse precisa do argumento correspondente.
class CaixaCrua implements Caixa {
  @override
  Object? get valor => 'cru';
}

typedef Ler = String Function(Caixa c);

String ler(Caixa c) => '${c.valor}';

List<T> semNulos<T>(List<T?> xs) {
  final r = <T>[];
  for (final x in xs) {
    if (x != null) r.add(x);
  }
  return r;
}

Ler? compor(List<Ler?>? fs) {
  if (fs == null) return null;
  final ps = semNulos(fs);
  if (ps.isEmpty) return null;
  return (Caixa c) => ps.map((f) => f(c)).join(',');
}

// Campo e parâmetro crus, e uma coleção de tipo cru.
class Registro {
  final List<Caixa> caixas;
  final Map<String, Caixa> porNome;
  Registro(this.caixas, this.porNome);

  String descrever(Caixa c) => '${c.valor}';
}

void main() {
  final f = compor(<Ler?>[ler, null, ler]);
  print(f!(CaixaInt()));
  print(f is Ler);
  print(semNulos<Ler>(<Ler?>[null, ler]).length);

  final cruas = <Caixa>[CaixaInt(), CaixaCrua()];
  print(cruas is List<Caixa<dynamic>>);
  print(cruas.length);

  final r = Registro(cruas, <String, Caixa>{'a': CaixaInt()});
  print(r.descrever(r.porNome['a']!));
  print(r.caixas is List<Caixa>);
  print(r.porNome is Map<String, Caixa<dynamic>>);

  final Object o = CaixaCrua();
  print(o is Caixa);
  print((o as Caixa).valor);
  print(o is Caixa<int>);

  // `dynamic` explícito tem de dar a mesma receita que o cru.
  final Object g = <Caixa<dynamic>>[];
  print(g is List<Caixa>);
}
