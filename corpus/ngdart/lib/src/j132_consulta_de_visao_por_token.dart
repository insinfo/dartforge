import 'package:ngdart/angular.dart';

/// `@ViewChildren(Token)`/`@ViewChild(Token)` cujo token outra diretiva
/// fornece (`ExistingProvider`), com resultados em `*`: o campo
/// `_viewQuery_Token_i_isDirty`, a atualização no `afterNodes` (depois das
/// consultas de conteúdo) e as marcas no `dirtyParentQueriesInternal` (o
/// `FocusableActivateItem` do `menu_item_groups`).
abstract class J132Item {}

@Directive(
  selector: '[j132Marca]',
  providers: [ExistingProvider(J132Item, J132Marca)],
)
class J132Marca implements J132Item {}

@Component(
  selector: 'j132-usa',
  template: '''<div *ngFor="let g of grupos">
  <template [ngIf]="g > 0"><span *ngFor="let i of lista" j132Marca>{{i}}</span></template>
</div>
<p *ngIf="x" j132Marca></p>''',
  directives: [J132Marca, NgFor, NgIf],
)
class J132Usa {
  bool x = true;
  List<int> grupos = [1];
  List<int> lista = [1];

  @ViewChildren(J132Item)
  List<J132Item>? itens;

  @ViewChild(J132Item)
  J132Item? primeiro;
}
