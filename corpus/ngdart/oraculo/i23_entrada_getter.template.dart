// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i23_entrada_getter.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i23_entrada_getter.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I23EntradaGetter = const [];

class ViewI23EntradaGetter0 extends import0.ComponentView<import1.I23EntradaGetter> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI23EntradaGetter0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('i23-entrada-getter'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i23_entrada_getter.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.rotulo;
    if (import9.checkBinding(this._expr_0, currVal_0, 'rotulo', 'package:corpus_ngdart/src/i23_entrada_getter.html')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i23_entrada_getter.html:3:19 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.vazio;
    if (import9.checkBinding(this._expr_1, currVal_1, 'vazio', 'package:corpus_ngdart/src/i23_entrada_getter.html')) {
      import8.setProperty(this._el_0, 'hidden', currVal_1) /* REF:package:corpus_ngdart/src/i23_entrada_getter.html:20:36 */;
      this._expr_1 = currVal_1;
    }
    this._textBinding_1.updateTextWithPrimitive(_ctx.total) /* REF:package:corpus_ngdart/src/i23_entrada_getter.html:37:46 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I23EntradaGetter, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I23EntradaGetterNgFactory = ComponentFactory<import1.I23EntradaGetter>('i23-entrada-getter', viewFactory_I23EntradaGetterHost0);
ComponentFactory<import1.I23EntradaGetter> get I23EntradaGetterNgFactory {
  return _I23EntradaGetterNgFactory;
}

ComponentFactory<import1.I23EntradaGetter> createI23EntradaGetterFactory() {
  return ComponentFactory('i23-entrada-getter', viewFactory_I23EntradaGetterHost0);
}

final List<Object> styles$I23EntradaGetterHost = const [];

class _ViewI23EntradaGetterHost0 extends import11.HostView<import1.I23EntradaGetter> {
  @override
  void build() {
    this.componentView = ViewI23EntradaGetter0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I23EntradaGetter();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I23EntradaGetter> viewFactory_I23EntradaGetterHost0() {
  return _ViewI23EntradaGetterHost0();
}
