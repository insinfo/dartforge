// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j134_hostbinding_estatico_de_diretiva.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j134_hostbinding_estatico_de_diretiva.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/check_binding.dart' as import4;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import5;

class J134FonteNgCd extends import0.DirectiveChangeDetector {
  final import1.J134Fonte instance;
  Object? _expr_1;
  J134FonteNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    bool firstCheck = view.firstCheck;
    if (firstCheck) {
      if ((import1.J134Fonte.cursor != null)) {
        el.style.setProperty('cursor', import1.J134Fonte.cursor?.toString());
      }
    }
    final currVal_1 = this.instance.ativa;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateClassBindingNonHtml(el, 'ativa', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}
