import 'package:ngdart/angular.dart';

/// `viewProviders:` (o `MaterialDropdownSelectComponent`): provedor privado
/// do elemento do componente, escrito na hospedeira (preguiçoso, no
/// `injectorGetInternal`); na visão dele, o `@Host()` de um token deles vai
/// ao injetor em vez de ficar `null`.
class J137Modelo<T> {
  J137Modelo(Object dono);
}

class J137Outro {}

J137Modelo<dynamic> j137DoCaixa(J137Caixa c) => J137Modelo<dynamic>(c);

@Directive(selector: '[j137Leitor]')
class J137Leitor {
  final J137Modelo? modelo;
  final J137Outro? outro;

  J137Leitor(@Optional() @Host() this.modelo, @Optional() @Host() this.outro);
}

@Component(
  selector: 'j137-caixa',
  template: '<p j137Leitor>x</p>',
  directives: [J137Leitor],
  viewProviders: [
    FactoryProvider<J137Modelo>(J137Modelo, j137DoCaixa, deps: [J137Caixa]),
  ],
)
class J137Caixa {}
