import 'package:ngdart/angular.dart';

/// Sonda de recusa: `<template ngFor let-item [ngForOf]>` escrito à mão —
/// só o `<template>` sem diretiva é traduzido.
@Component(
  selector: 'i84-template-com-diretiva',
  templateUrl: 'i84_template_com_diretiva.html',
  directives: [coreDirectives],
)
class I84TemplateComDiretiva {
  List<String> itens = ['a', 'b'];
}
