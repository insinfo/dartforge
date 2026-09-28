// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j46_varias_ngcd.dart';
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import0;
import 'j46_varias_ngcd.dart' as import1;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/runtime/check_binding.dart' as import4;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import5;

class J46UmNgCd extends import0.DirectiveChangeDetector {
  final import1.J46Um instance;
  Object? _expr_0;
  J46UmNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.um;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateClassBindingNonHtml(el, 'um', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}

class J46DoisNgCd extends import0.DirectiveChangeDetector {
  final import1.J46Dois instance;
  Object? _expr_0;
  Object? _expr_1;
  J46DoisNgCd(this.instance);
  void detectHostChanges(import2.RenderView view, import3.Element el) {
    final currVal_0 = this.instance.papel;
    if (import4.checkBinding(this._expr_0, currVal_0, null, null)) {
      import5.updateAttribute(el, 'role', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.largura;
    if (import4.checkBinding(this._expr_1, currVal_1, null, null)) {
      el.style.setProperty('width', ((currVal_1 == null) ? null : (currVal_1.toString() + 'px')));
      this._expr_1 = currVal_1;
    }
  }
}
