import 'package:ngdart/angular.dart';

/// Diretiva de `<template>` (o `liPaginationPages` do limitless_ui).
@Directive(selector: '[j54-marca]')
class J54Marca {
  final TemplateRef molde;

  J54Marca(this.molde);
}

/// `<template>` vazio: a visão embutida não tem nó nenhum.
@Component(
  selector: 'j54-molde-vazio',
  templateUrl: 'j54_molde_vazio.html',
  directives: [J54Marca],
)
class J54MoldeVazio {}
