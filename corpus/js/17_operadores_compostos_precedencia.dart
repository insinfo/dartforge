// Precedência e associatividade de operadores, i++ vs ++i, a = b = c, compostos em índices e campos com efeitos únicos.
class Caixa {
  int valor = 0;
  List<int> itens = [10, 20, 30];
  Map<String, int> m = {'a': 1};
  int? talvez;
}

int contador = 0;
Caixa caixaUnica = Caixa();
Caixa obtemCaixa() {
  contador++;
  print('obtemCaixa #$contador');
  return caixaUnica;
}

int indice() {
  contador++;
  print('indice #$contador');
  return 1;
}

void main() {
  print(2 + 3 * 4 - 1);
  print(2 * 3 + 4 * 5);
  print(10 - 2 - 3);
  print(2 * 3 ~/ 4);
  print(100 ~/ 10 * 2);
  print(-2 * 3);
  print(-(2 * 3));
  print(2 + -3);
  print(-2 + -3);
  print(1 < 2 == true);
  print(1 + 2 < 4 && 5 > 3);
  print(true || false && false);
  print((true || false) && false);
  print(false ? 1 : true ? 2 : 3);
  print(true ? false ? 1 : 2 : 3);
  int? nulo;
  print(nulo ?? 1 ?? 2);
  print(null ?? null ?? 3);
  print(nulo ?? 5);
  print(1 + 2 == 3 ? 'sim' : 'não');
  print(3 is int && 3 > 2);
  print((3 as num) + 1);
  print(3 > 2 == 3 > 1);
  print(5 - 3 - 1 == 1);
  print(2 * 3 % 4 == 2);
  print(7 % 4 * 2);
  print(1 << 2 + 1);
  print((1 << 2) + 1);
  print(4 & 2 | 1);
  print(4 | 2 ^ 3);
  print(4 ^ 2 & 3);
  print(1 + 2 << 1 > 5);
  print(!true == false);
  print(!(1 == 2));
  print(-3.abs());
  print((-3).abs());
  print(-3 % 2);
  print(-(3 % 2));
  print(2 - -2);
  print(2 - - 2);
  print([1, 2, 3][1] + 1);
  print('ab' 'c'.length);
  print(('ab' 'c').length);
  print('x'.length + 'yz'.length * 2);

  var i = 5;
  var a = i++;
  print('$a $i');
  var b = ++i;
  print('$b $i');
  var c = i--;
  print('$c $i');
  var d = --i;
  print('$d $i');
  print(i++ + i++);
  print(i);
  print(++i + ++i);
  print(i);
  print(i++ + ++i);
  print(i);
  var j = 0;
  print([j++, j++, ++j, j]);
  print(j);
  var lista = [0, 0, 0];
  var k = 0;
  lista[k++] = k++;
  print('$lista $k');
  lista[k] = k = 1;
  print('$lista $k');

  int x, y, z;
  x = y = z = 7;
  print('$x $y $z');
  x = (y = 3) + (z = 4);
  print('$x $y $z');
  x += y *= 2;
  print('$x $y');
  x = y += z -= 1;
  print('$x $y $z');

  var caixa = Caixa();
  caixa.valor += 5;
  caixa.valor *= 3;
  caixa.valor -= 1;
  caixa.valor ~/= 2;
  caixa.valor %= 4;
  print(caixa.valor);
  caixa.valor++;
  print(caixa.valor++);
  print(++caixa.valor);
  print(caixa.valor);
  caixa.itens[1] += 5;
  caixa.itens[2] *= 2;
  caixa.itens[0]++;
  print(caixa.itens);
  caixa.m['a'] = caixa.m['a']! + 1;
  print(caixa.m);
  caixa.m['b'] ??= 9;
  caixa.m['b'] ??= 100;
  print(caixa.m);
  caixa.talvez ??= 1;
  caixa.talvez = caixa.talvez! + 1;
  print(caixa.talvez);

  contador = 0;
  obtemCaixa().valor += 10;
  print(caixaUnica.valor);
  obtemCaixa().itens[indice()] += 100;
  print(caixaUnica.itens);
  obtemCaixa().valor++;
  print(caixaUnica.valor);
  obtemCaixa().talvez ??= 5;
  obtemCaixa().talvez ??= 6;
  print(caixaUnica.talvez);
  print(contador);
  var r = obtemCaixa().itens[indice()]++;
  print('$r ${caixaUnica.itens}');
  print(contador);
  var r2 = ++obtemCaixa().itens[indice()];
  print('$r2 ${caixaUnica.itens}');
  print(contador);
  nulo ??= 3;
  nulo += 1;
  print(nulo);
  var s = 'a';
  s += 'b';
  s *= 2;
  print(s);
  var l = [1];
  l += [2, 3];
  print(l);
  var t = 10;
  t -= t -= 3;
  print(t);
  var u = 2;
  u *= u += 1;
  print(u);
  var v = 1;
  v = v++ + v;
  print(v);
  var w = 1;
  w = ++w + w;
  print(w);
}
