// O `?.` no meio da cadeia encurta a atribuição inteira (§17.23): em
// `a?.b.c = v`, com `a` null, nem o setter nem `v` são avaliados e o valor é
// null. Era um *segfault* no `EnclosingTypeParameterReferenceFlag.perform`
// do analyzer (`field.getter2?.firstFragment.flag = r`): o setter escrevia
// no objeto vazio do runtime.
class Fragmento {
  bool marca = true;
  int conta = 0;
  final List<int> itens = [0, 0];
  Fragmento? proximo;
}

class Elemento {
  final Fragmento? _f;
  Elemento(this._f);
  Fragmento get primeiro => _f!;
  Fragmento ultimo() => _f!;
}

class Dono {
  Elemento? getter;
  Dono(this.getter);
}

int _efeitos = 0;
T efeito<T>(T v) {
  _efeitos++;
  return v;
}

void main() {
  final donos = [Dono(Elemento(Fragmento())), Dono(null)];
  for (final d in donos) {
    final r1 = d.getter?.primeiro.marca = efeito(false);
    final r2 = d.getter?.primeiro.conta += efeito(3);
    final r5 = d.getter?.primeiro.itens[efeito(1)] = efeito(7);
    final r6 = d.getter?.primeiro.itens[0] += 5;
    final r7 = d.getter?.ultimo().proximo ??= Fragmento();
    final r8 = d.getter?.primeiro.proximo?.conta = efeito(9);
    final r9 = d.getter?.primeiro.proximo!.itens[1] = 4;
    print('$r1 $r2 $r5 $r6 ${r7 != null} $r8 $r9');
    final f = d.getter?.primeiro;
    print('${f?.marca} ${f?.conta} ${f?.itens} ${f?.proximo?.conta} '
        '${f?.proximo?.itens} efeitos=$_efeitos');
  }
  Elemento? e;
  e?.primeiro.marca = efeito(true);
  e?.primeiro.itens[efeito(0)] = efeito(1);
  print('efeitos=$_efeitos');
  // O `!` é seletor da cadeia: `e?.primeiro.proximo!.conta` com `e` null
  // vale null, sem checar o `!`.
  print(e?.primeiro.proximo!.conta);
  print(e?.ultimo().proximo!.itens[0]);
  print((e?.primeiro.proximo!.conta ?? 7) + 1);
  final g = Elemento(Fragmento()..proximo = Fragmento());
  print(g.primeiro.proximo!.conta);
  try {
    print((e?.primeiro)!.conta);
  } on TypeError catch (x) {
    print('parênteses encerram a cadeia: $x');
  }
}
