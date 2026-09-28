// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j118_atributos_de_diretiva.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j118_atributos_de_diretiva.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/check_binding.dart' as import4;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import5;

class J118ItemNgCd extends import0.DirectiveChangeDetector {
  final import1.J118Item instance;
  Object? _expr_0;
  J118ItemNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.ativo;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}
