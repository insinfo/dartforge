import 'package:ngdart/angular.dart';

/// `<ng-content>` reprojetado entre textos: o `createFlatArrayForProjectNodes`
/// concatena as listas com `..addAll(..)`, e o dart_style quebra a cascata em
/// várias linhas, tanto no `createAndProject` quanto no
/// `initRootNodesAndSubscriptions` (raiz e visão embutida). Um evento que
/// atribui a um campo herdado ou a um setter escreve `_ctx.campo`, e o
/// `ChangeDetectorRef` de uma diretiva no elemento de um filho é a visão
/// dele (`componentView`).
@Component(
  selector: 'j120-caixa',
  template: '<div><ng-content select="[cabeca]"></ng-content></div>'
      '<ng-content></ng-content>',
)
class J120Caixa {}

class J120Base {
  bool visivel = true;
}

@Directive(selector: '[j120Det]')
class J120Det {
  J120Det(ChangeDetectorRef cd);
}

@Component(
  selector: 'j120-usa',
  template: '''<j120-caixa j120Det>a<ng-content></ng-content>b</j120-caixa>
<template [ngIf]="visivel">c<ng-content select="[x]"></ng-content>d</template>
<button (focus)="visivel = false" (blur)="aberto = true">e</button>''',
  directives: [J120Caixa, J120Det, NgIf],
)
class J120Usa extends J120Base {
  set aberto(bool v) {}
}

@Component(
  selector: 'j120-raiz',
  template: 'antes<ng-content></ng-content>depois',
)
class J120Raiz {}

/// Hospedeira com `ViewContainer` e provedor preguiçoso: o campo com
/// inicializador sai antes do `_appEl_0` (`dart_emitter.dart:198-205`).
class J120Servico {}

J120Servico servico() => J120Servico();

@Component(
  selector: 'j120-conteiner',
  template: 'x',
  providers: [FactoryProvider(J120Servico, servico)],
)
class J120Conteiner {
  J120Conteiner(ViewContainerRef vc);
}
