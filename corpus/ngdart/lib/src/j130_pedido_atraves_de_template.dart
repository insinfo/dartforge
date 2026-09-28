import 'package:ngdart/angular.dart';

/// Pedido a um provedor preguiçoso do filho que atravessa a raiz de uma
/// visão embutida: `_getDependency` pede com `eager: false` (`_isViewRoot`),
/// o provedor é transformado na visita (muda de posição) mas segue
/// preguiçoso, e o nó de baixo lê o campo pela cadeia de `parentView`.
class J130A {}

class J130B {}

J130A fabricaA() => J130A();

J130B fabricaB() => J130B();

@Component(
  selector: 'j130-caixa',
  template: '<ng-content></ng-content>',
  providers: [
    FactoryProvider(J130A, fabricaA),
    FactoryProvider(J130B, fabricaB),
  ],
)
class J130Caixa {}

@Directive(selector: '[j130Pede]')
class J130Pede {
  J130Pede(J130B b);
}

@Component(
  selector: 'j130-usa',
  template: '<j130-caixa><div *ngIf="x"><span j130Pede></span></div></j130-caixa>',
  directives: [J130Caixa, J130Pede, NgIf],
)
class J130Usa {
  bool x = true;
}
