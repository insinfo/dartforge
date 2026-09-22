// ~/, % e remainder com combinações de sinais em int e double.
void main() {
  print(7 ~/ 2);
  print(-7 ~/ 2);
  print(7 ~/ -2);
  print(-7 ~/ -2);
  print(7 % 2);
  print(-7 % 2);
  print(7 % -2);
  print(-7 % -2);
  print(7.remainder(2));
  print((-7).remainder(2));
  print(7.remainder(-2));
  print((-7).remainder(-2));
  print(-7.remainder(2));
  print(8 % 4);
  print(-8 % 4);
  print(0 % 5);
  print(0 ~/ 5);
  print(5 % 7);
  print(-5 % 7);
  print(5 % -7);
  print(-1 % 3);
  print(-1 ~/ 3);
  print(-3 ~/ 3);
  print(-4 ~/ 3);
  print(1 ~/ 3);
  print(99 ~/ 10);
  print(-99 ~/ 10);
  print(99 % 10);
  print(-99 % 10);
  print(7.5 % 2);
  print(-7.5 % 2);
  print(7.5 % -2);
  print(7.5.remainder(2));
  print((-7.5).remainder(2));
  print(7.5 ~/ 2);
  print(-7.5 ~/ 2);
  print(7.5 ~/ -2);
  print(5.5 % 1);
  print(-5.5 % 1);
  print(5.5.remainder(1));
  print((-5.5).remainder(1));
  print(10 % 3.75);
  print(-10 % 3.75);
  print(0.75 % 0.5);
  for (var a in [-7, 7]) {
    for (var b in [-3, 3]) {
      print('$a ~/ $b = ${a ~/ b}, $a % $b = ${a % b}, rem ${a.remainder(b)}');
    }
  }
  print((-7 ~/ 2) * 2 + (-7).remainder(2));
  print((-7 ~/ 2) * 2 + (-7 % 2));
  print(((-7) - (-7 % 2)) ~/ 2);
  print(2.5 ~/ 0.5);
  print(-2.5 ~/ 0.5);
  print(7.25 ~/ 2.5);
  try {
    print(1 ~/ 0);
  } catch (e) {
    print('lançou');
  }
  print((7.5 % 0).isNaN);
  print((7.5 ~/ 1).isEven);
  print(-9 ~/ 4);
  print(-9 % 4);
  print(9 ~/ -4);
  print(9 % -4);
  print((-9).remainder(4));
  print(9.remainder(-4));
  print(-2 % 1000000);
  print(-1000001 % 1000000);
  print(-1000001 ~/ 1000000);
  print(1000001 ~/ -1000000);
  print(-0.5 ~/ 1);
  print(0.5 ~/ 1);
  print(-1.5 % 1);
  print(-1.5.remainder(1));
}
