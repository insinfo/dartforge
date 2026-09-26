// Gravação direta em `List<int>`/`List<double>`/`List<bool>`
// (`lower/tipados.rs`): só quando o `E` reificado da lista aceita o valor;
// uma lista de subtipo vista pelo supertipo (covariância) fica com o `[]=`
// do SDK e o mesmo erro da VM.
void gravarInt(List<int> l, int v) => l[0] = v;
void gravarBool(List<bool> l, bool v) => l[0] = v;
void gravarDouble(List<double> l, double v) => l[0] = v;

void tentar(String nome, void Function() f) {
  try {
    f();
    print('$nome: ok');
  } on TypeError catch (e) {
    print('$nome: TypeError ${e.runtimeType}');
  } catch (e) {
    print('$nome: ${e.runtimeType}');
  }
}

void main() {
  final bs = List<bool>.filled(5, true);
  for (var i = 0; i < bs.length; i += 2) {
    bs[i] = false;
  }
  print(bs);
  var verdadeiros = 0;
  for (var i = 0; i < bs.length; i++) {
    if (bs[i]) verdadeiros++;
  }
  print(verdadeiros);
  final nunca = <Never>[];
  tentar('Never vazia', () => gravarInt(nunca, 1));
  final List<Object?> objs = [1, 'a', null];
  tentar('Object', () {
    (objs as List<Object?>)[0] = true;
    print(objs);
  });
  final dyn = <dynamic>[0, 0];
  tentar('dynamic int', () => gravarInt(dyn.cast<int>(), 5));
  final List<num> nums = <int>[1, 2, 3];
  tentar('int como num recebe double', () => nums[0] = 1.5);
  tentar('int grava int', () => gravarInt(nums as List<int>, 9));
  print(nums);
  final List<int?> anulaveis = [1, null];
  anulaveis[1] = 7;
  print(anulaveis);
  final List<Object> caixas = <bool>[true, false];
  tentar('bool como Object recebe int', () => caixas[0] = 3);
  tentar('bool grava bool', () => gravarBool(caixas as List<bool>, false));
  print(caixas);
  final ds = <double>[1, 2];
  gravarDouble(ds, 2.5);
  print(ds);
  final List<Comparable<num>> comps = <double>[1.0];
  tentar('double como Comparable recebe int', () => comps[0] = 3);
  final fixa = List<int>.unmodifiable([1, 2]);
  tentar('imutavel', () => gravarInt(fixa, 3));
}
