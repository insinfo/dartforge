import 'package:ngdart/angular.dart';

/// O `MaterialTreeDropdownComponent`/`MaterialTreeGroupFlatRadioComponent`:
/// filho com `viewProviders:` e sem nós filhos (o apelido privado entra no
/// mesmo `ProviderNode`, no `injectorGetInternal` do nó; o construtor do
/// filho, com `@SkipSelf()`, vai ao injetor), token genérico no construtor
/// (`J138Raiz<T>` pede `J138Raiz`) e `@ContentChildren` dinâmico cujo
/// resultado é componente `onPush` (o `ChangeDetectorRef` dele no
/// `View.queryChangeDetectorRefs`).
abstract class J138Raiz<T> {}

@Component(
  selector: 'j138-arvore',
  template: 'a',
  viewProviders: [ExistingProvider(J138Raiz, J138Arvore)],
)
class J138Arvore<T> implements J138Raiz<T> {
  final J138Raiz<T>? pai;

  J138Arvore(@Optional() @SkipSelf() this.pai);
}

@Component(
  selector: 'j138-item',
  template: 'i',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J138Item {}

@Component(
  selector: 'j138-grupo',
  template: '<ng-content></ng-content>',
)
class J138Grupo {
  @ContentChildren(J138Item)
  List<J138Item>? itens;
}

@Component(
  selector: 'j138-caixa',
  template: '''
<j138-arvore></j138-arvore>
<j138-grupo>
  <j138-item *ngFor="let x of xs"></j138-item>
</j138-grupo>''',
  directives: [J138Arvore, J138Grupo, J138Item, NgFor],
)
class J138Caixa<T> {
  final J138Raiz<T> raiz;
  List<int> xs = [1, 2];

  J138Caixa(this.raiz);
}
