// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c16_atributo_interpolado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c16_atributo_interpolado.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$C16AtributoInterpolado = const [];

class ViewC16AtributoInterpolado0 extends import0.ComponentView<import1.C16AtributoInterpolado> {
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  Object? _expr_4;
  Object? _expr_5;
  Object? _expr_7;
  late final import2.DivElement _el_0;
  late final import2.HtmlElement _el_1;
  late final import2.HtmlElement _el_3;
  late final import2.HtmlElement _el_5;
  late final import2.HtmlElement _el_7;
  late final import2.HtmlElement _el_9;
  static import3.ComponentStyles? _componentStyles;
  ViewC16AtributoInterpolado0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('c16-atributo-interpolado'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c16_atributo_interpolado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendSpan(doc, this._el_0);
    final _text_2 = import7.appendText(this._el_1, '1');
    this._el_3 = import7.appendElement<import2.HtmlElement>(doc, this._el_0, 'p');
    final _text_4 = import7.appendText(this._el_3, '2');
    this._el_5 = import7.appendElement<import2.HtmlElement>(doc, this._el_0, 'i');
    final _text_6 = import7.appendText(this._el_5, '3');
    this._el_7 = import7.appendElement<import2.HtmlElement>(doc, this._el_0, 'b');
    final _text_8 = import7.appendText(this._el_7, '4');
    this._el_9 = import7.appendElement<import2.HtmlElement>(doc, this._el_0, 'em');
    final _text_10 = import7.appendText(this._el_9, '5');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      import7.setProperty(this._el_0, 'hidden', false) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:44:60 */;
    }
    final currVal_1 = import8.interpolateString1('a ', _ctx.nome, ' b');
    if (import9.checkBinding(this._expr_1, currVal_1, 'a {{nome}} b', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      import7.setProperty(this._el_0, 'title', currVal_1) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:5:25 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = import8.interpolateString1('x ', _ctx.cls, '');
    if (import9.checkBinding(this._expr_2, currVal_2, 'x {{cls}}', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      this.updateChildClass(this._el_0, currVal_2) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:26:43 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.n;
    if (import9.checkBinding(this._expr_3, currVal_3, '{{n}}', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      import7.setProperty(this._el_1, 'title', import8.interpolate0(currVal_3)) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:67:80 */;
      this._expr_3 = currVal_3;
    }
    final currVal_4 = _ctx.n;
    if (import9.checkBinding(this._expr_4, currVal_4, 'n={{n}}', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      import7.setProperty(this._el_3, 'title', import8.interpolate1('n=', currVal_4, '')) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:92:107 */;
      this._expr_4 = currVal_4;
    }
    final currVal_5 = import8.interpolate2('', _ctx.a, '-', _ctx.n, '');
    if (import9.checkBinding(this._expr_5, currVal_5, '{{a}}-{{n}}', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      import7.setProperty(this._el_5, 'title', currVal_5) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:116:135 */;
      this._expr_5 = currVal_5;
    }
    if (firstCheck) {
      import7.setProperty(this._el_7, 'id', import8.interpolateString0(_ctx.fixo)) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:144:157 */;
    }
    final currVal_7 = import8.interpolateString0(_ctx.nome);
    if (import9.checkBinding(this._expr_7, currVal_7, '{{nome}}', 'package:corpus_ngdart/src/c16_atributo_interpolado.html')) {
      import7.setProperty(this._el_9, 'title', currVal_7) /* REF:package:corpus_ngdart/src/c16_atributo_interpolado.html:167:183 */;
      this._expr_7 = currVal_7;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C16AtributoInterpolado, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C16AtributoInterpoladoNgFactory = ComponentFactory<import1.C16AtributoInterpolado>('c16-atributo-interpolado', viewFactory_C16AtributoInterpoladoHost0);
ComponentFactory<import1.C16AtributoInterpolado> get C16AtributoInterpoladoNgFactory {
  return _C16AtributoInterpoladoNgFactory;
}

ComponentFactory<import1.C16AtributoInterpolado> createC16AtributoInterpoladoFactory() {
  return ComponentFactory('c16-atributo-interpolado', viewFactory_C16AtributoInterpoladoHost0);
}

final List<Object> styles$C16AtributoInterpoladoHost = const [];

class _ViewC16AtributoInterpoladoHost0 extends import11.HostView<import1.C16AtributoInterpolado> {
  @override
  void build() {
    this.componentView = ViewC16AtributoInterpolado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C16AtributoInterpolado();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.C16AtributoInterpolado> viewFactory_C16AtributoInterpoladoHost0() {
  return _ViewC16AtributoInterpoladoHost0();
}
