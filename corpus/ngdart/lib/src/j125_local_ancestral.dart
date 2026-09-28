import 'dart:collection';

import 'package:ngdart/angular.dart';

/// Locais de `*ngFor` lidos em visões aninhadas: o `getLocal` do
/// `ViewNameResolver` sobe pelas `declarationElement.view` e lê
/// `unsafeCast<_ViewX1>((this.parentView!)).locals['\$implicit']`, com o
/// `unsafeCast<T>` do tipo do local (o `single` da coleção, com os
/// argumentos substituídos; `dynamic` sem cast).
/// Coleção que herda o `single` (`ListBase` → `ListMixin`): o tipo do
/// `$implicit` é o retorno dele com os argumentos substituídos pela
/// hierarquia (`getIterableElementType`, `analyzed_class.dart:39-42`).
class J125Lista<T> extends ListBase<T> {
  final List<T> _base = [];
  @override
  int get length => _base.length;
  @override
  set length(int n) => _base.length = n;
  @override
  T operator [](int i) => _base[i];
  @override
  void operator []=(int i, T v) => _base[i] = v;
}

class J125Grupo<T> {
  List<T> itens = [];
  String nome = '';
  bool visivel = true;
}

@Component(
  selector: 'j125-usa',
  template: '''<div *ngFor="let grupo of grupos; let i = index">
  <div *ngIf="grupo.visivel">
    <span *ngFor="let item of grupo.itens; let j = index">{{i}}-{{j}} {{item}} {{grupo.nome}}</span>
  </div>
  <template ngFor let-x [ngForOf]="lista" let-k="index">{{x}}{{k}}{{grupo.nome}}</template>
</div>''',
  directives: [NgFor, NgIf],
)
class J125Usa {
  List<J125Grupo> grupos = [];
  J125Lista<String> lista = J125Lista<String>();
}
