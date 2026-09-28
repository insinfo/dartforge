import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@ContentChildren(Token, descendants: true)` com resultados em `*`: o
/// campo `_query_Token_n_i_isDirty`, a atualização com `mapNestedViews` no
/// bloco dos ganchos de conteúdo e o `dirtyParentQueriesInternal` de cada
/// visão embutida com resultado (`compile_query.dart`). O token vem de um
/// `ExistingProvider` de outra diretiva (o `FocusableItem` do
/// `material_select`).
abstract class J128Item {}

@Directive(
  selector: '[j128Marcador]',
  providers: [ExistingProvider(J128Item, J128Marcador)],
)
class J128Marcador implements J128Item {}

class J128Servico {}

J128Servico fabricaDeServico() => J128Servico();

/// Provedor que ninguém pede: preguiçoso, campo com inicializador alocado
/// quando o nó é criado — antes do campo sujo de uma consulta cujo primeiro
/// resultado em `*` vem depois.
@Directive(
  selector: '[j128Prov]',
  providers: [FactoryProvider(J128Servico, fabricaDeServico)],
)
class J128Prov {}

@Directive(selector: '[j128Lista]')
class J128Lista {
  @ContentChildren(J128Item, descendants: true)
  List<J128Item> itens = [];
}

@Directive(selector: '[j128Um]')
class J128Um {
  @ContentChild(J128Item)
  J128Item? primeiro;
}

@Component(
  selector: 'j128-caixa',
  template: '<ng-content></ng-content>',
)
class J128Caixa {
  @ContentChildren(J128Item, descendants: true)
  List<J128Item> itens = [];
}

@Component(
  selector: 'j128-usa',
  template: '''<div j128Lista><span j128Marcador>a</span>
  <template [ngIf]="x"><b *ngFor="let i of lista" j128Marcador></b></template></div>
<p j128Um><i *ngIf="x" j128Marcador></i></p>
<j128-caixa><em *ngIf="x" j128Marcador></em></j128-caixa>
<div *ngIf="x"><section j128Lista><u *ngIf="x" j128Marcador></u></section></div>
<nav j128Prov j128Lista><a #r *ngIf="x" j128Marcador></a></nav>''',
  directives: [J128Marcador, J128Lista, J128Um, J128Caixa, J128Prov, NgIf, NgFor],
)
class J128Usa {
  bool x = true;
  List<int> lista = [1];

  @ViewChildren('r')
  List<Element> rs = [];
}
