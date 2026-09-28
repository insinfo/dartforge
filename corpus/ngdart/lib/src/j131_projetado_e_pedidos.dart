import 'package:ngdart/angular.dart';

/// `@ViewChild(Tipo)` cujo resultado é um filho `onPush` no conteúdo
/// projetado de outro filho, dentro de `*`: o `mapNestedViews` registra o
/// `ChangeDetectorRef` e a visão embutida marca a consulta no
/// `dirtyParentQueriesInternal`. E um nó que pede dois provedores
/// preguiçosos do filho de cima através de `*`: a ordem é a das
/// dependências dele.
@Component(
  selector: 'j131-alvo',
  template: 'a',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J131Alvo {}

@Component(
  selector: 'j131-lista',
  template: '<ng-content></ng-content>',
)
class J131Lista {}

class J131A {}

class J131B {}

J131A fabricaA() => J131A();

J131B fabricaB() => J131B();

@Component(
  selector: 'j131-caixa',
  template: '<ng-content></ng-content>',
  providers: [
    FactoryProvider(J131A, fabricaA),
    FactoryProvider(J131B, fabricaB),
  ],
)
class J131Caixa {}

@Directive(selector: '[j131Dois]')
class J131Dois {
  J131Dois(J131B b, J131A a);
}

@Component(
  selector: 'j131-usa',
  template: '''<j131-lista *ngIf="x"><j131-alvo></j131-alvo></j131-lista>
<j131-caixa><div *ngIf="x"><span j131Dois></span></div></j131-caixa>''',
  directives: [J131Alvo, J131Lista, J131Caixa, J131Dois, NgIf],
)
class J131Usa {
  bool x = true;

  @ViewChild(J131Alvo)
  J131Alvo? alvo;
}
