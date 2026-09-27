import 'package:ngdart/angular.dart';

class I65A {}

class I65A2 extends I65A {}

class I65B {}

class I65C {}

class I65D {}

const i65Modulo = [
  ClassProvider(I65A),
  [ClassProvider(I65B)],
];

/// Sonda: listas aninhadas, lista constante de topo, classe solta na lista e
/// provedor sobrescrito por outro de mesmo token.
@Component(
  selector: 'i65-provider-listas',
  template: '<p>x</p>',
  providers: [
    i65Modulo,
    [
      ClassProvider(I65C),
      [I65D],
    ],
    ClassProvider(I65A, useClass: I65A2),
  ],
)
class I65ProviderListas {}
