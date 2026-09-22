// Parâmetros `super.x` sem anotação herdam o tipo do construtor da superclasse:
// o literal `{}` passado a eles tem de virar Set, não Map.
class Base<E> {
  final Set<E> itens;
  final String rotulo;
  Base(this.itens, {this.rotulo = 'b'});
  Base.nomeado(String r, this.itens) : rotulo = r;
}

class Vista<E> extends Base<E> {
  Vista(super.itens, {super.rotulo});
  Vista.nomeada(super.r, super.itens) : super.nomeado();
}

class Conf {
  final Set<String> tags;
  final List<int> nums;
  Conf({Iterable<String>? tags, Iterable<int>? nums})
      : tags = Vista<String>(tags == null ? {} : tags.toSet()).itens,
        nums = Vista<int>({...?nums}).itens.toList();
}

void main() {
  print(Conf().tags);
  print(Conf(tags: ['a', 'b']).tags.where((t) => t != 'a'));
  print(Conf(nums: [3, 1]).nums);
  print(Vista({1, 2}, rotulo: 'x').rotulo);
  print(Vista({1}).rotulo);
  print(Vista.nomeada('n', {'z'}).itens);
  print(Vista.nomeada('n', {'z'}).rotulo);
  print(Vista(<int>{}).itens.isEmpty);
}
