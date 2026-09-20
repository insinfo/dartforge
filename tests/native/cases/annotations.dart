@Deprecated('Use NovoAnimal')
class Animal {
  int valor() => 1;
}

class Cachorro extends Animal {
  @override
  int valor() => 42;
}

@deprecated
int legado() => 7;

void main() {
  Animal animal = Cachorro();
  print(animal.valor());
  print(legado());
}
