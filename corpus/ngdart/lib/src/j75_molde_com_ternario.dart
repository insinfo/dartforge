import 'package:ngdart/angular.dart';

/// `<template [ngTemplateOutlet]="a == null ? null : b" [ngTemplateOutletValue]="c">`
/// (o modal do `li-datatable-select`): a expressão com `?:` não passa no
/// `isMicroExpression`, e o `<template>` escrito não volta ao texto.
/// `;` e `:` dentro de texto também não separam a microssintaxe.
@Component(
  selector: 'j75-molde-com-ternario',
  templateUrl: 'j75_molde_com_ternario.html',
  directives: [NgTemplateOutlet, NgFor, NgIf],
)
class J75MoldeComTernario {
  TemplateRef? modelo;
  TemplateRef? outro;
  Object contexto = 1;
  List<String> itens = ['a;b', 'c:d'];

  String rotulo(String x) => x;
}
