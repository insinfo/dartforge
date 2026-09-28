import 'package:ngdart/angular.dart';

class J83No {
  String rotulo = 'r';
  List<J83No> filhos = [];
}

/// `*ngFor` sobre um local sem tipo (`let-lista` de `<template>`, como a
/// árvore do `li-treeview`): o tipo da coleção ignora o local e cai no
/// membro do componente de mesmo nome (`_typeNgForLocals`).
@Component(
  selector: 'j83-ngfor-local-sem-tipo',
  templateUrl: 'j83_ngfor_local_sem_tipo.html',
  directives: [NgFor, NgTemplateOutlet],
)
class J83NgforLocalSemTipo {
  List<J83No> lista = [];
  dynamic soltos = [];
  Map<String, dynamic> contexto = {};
}
