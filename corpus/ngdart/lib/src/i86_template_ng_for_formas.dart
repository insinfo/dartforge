import 'package:ngdart/angular.dart';

/// Sonda: `<template ngFor>` escrito à mão com `trackBy` antes de `ngForOf`
/// e `let-i="index"` — a ordem e o `REF` de cada ligação.
@Component(
  selector: 'i86-template-ng-for-formas',
  templateUrl: 'i86_template_ng_for_formas.html',
  directives: [coreDirectives],
)
class I86TemplateNgForFormas {
  List<String> itens = ['a', 'b'];
  Object rastrear(int i, dynamic x) => x as Object;
}
