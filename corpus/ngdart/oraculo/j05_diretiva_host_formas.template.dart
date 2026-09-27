// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j05_diretiva_host_formas.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j05_diretiva_host_formas.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/check_binding.dart' as import4;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import5;

class J05DiretivaHostFormasNgCd extends import0.DirectiveChangeDetector {
  final import1.J05DiretivaHostFormas instance;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  J05DiretivaHostFormasNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.titulo;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.setProperty(el, 'title', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.papel;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      import5.updateAttribute(el, 'role', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = this.instance.id;
    if (import4.checkBinding(this._expr_2, currVal_2, null, null)) {
      import5.setProperty(el, 'id', currVal_2);
      this._expr_2 = currVal_2;
    }
    final currVal_3 = this.instance.ativo;
    if (import4.checkBinding(this._expr_3, currVal_3, null, null)) {
      import5.updateClassBindingNonHtml(el, 'ativo', currVal_3);
      this._expr_3 = currVal_3;
    }
  }
}
