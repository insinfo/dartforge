// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i92_seguranca_attr_estilo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i92_seguranca_attr_estilo.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/src/security/safe_html_adapter.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I92SegurancaAttrEstilo = const [];

class ViewI92SegurancaAttrEstilo0 extends import0.ComponentView<import1.I92SegurancaAttrEstilo> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  Object? _expr_5;
  late final import2.AnchorElement _el_0;
  late final import2.IFrameElement _el_2;
  late final import2.DivElement _el_3;
  late final import2.DivElement _el_4;
  late final import2.HtmlElement _el_5;
  late final import2.DivElement _el_6;
  static import3.ComponentStyles? _componentStyles;
  ViewI92SegurancaAttrEstilo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i92-seguranca-attr-estilo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i92_seguranca_attr_estilo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_1 = import7.appendText(this._el_0, 'a');
    this._el_2 = import7.appendElement<import2.IFrameElement>(doc, parentRenderNode, 'iframe');
    this._el_3 = import7.appendDiv(doc, parentRenderNode);
    this._el_4 = import7.appendDiv(doc, parentRenderNode);
    this._el_5 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'img');
    this._el_6 = import7.appendDiv(doc, parentRenderNode);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.url;
    if (import8.checkBinding(this._expr_0, currVal_0, 'url', 'package:corpus_ngdart/src/i92_seguranca_attr_estilo.html')) {
      import7.updateAttribute(this._el_0, 'href', import9.sanitizeUrl(currVal_0)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:3:20 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.url;
    if (import8.checkBinding(this._expr_1, currVal_1, 'url', 'package:corpus_ngdart/src/i92_seguranca_attr_estilo.html')) {
      import7.updateAttribute(this._el_2, 'src', import9.sanitizeResourceUrl(currVal_1)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:34:50 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.estilo;
    if (import8.checkBinding(this._expr_2, currVal_2, 'estilo', 'package:corpus_ngdart/src/i92_seguranca_attr_estilo.html')) {
      import7.updateAttribute(this._el_3, 'style', import9.sanitizeStyle(currVal_2)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:65:86 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.estilo;
    if (import8.checkBinding(this._expr_3, currVal_3, 'estilo', 'package:corpus_ngdart/src/i92_seguranca_attr_estilo.html')) {
      import7.setProperty(this._el_4, 'style', import9.sanitizeStyle(currVal_3)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:98:114 */;
      this._expr_3 = currVal_3;
    }
    if (firstCheck) {
      if ((_ctx.fixo != null)) {
        import7.updateAttribute(this._el_5, 'src', import9.sanitizeUrl(_ctx.fixo)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:126:143 */;
      }
    }
    final currVal_5 = _ctx.url;
    if (import8.checkBinding(this._expr_5, currVal_5, 'url', 'package:corpus_ngdart/src/i92_seguranca_attr_estilo.html')) {
      import7.updateAttribute(this._el_6, 'innerHTML', import9.sanitizeHtml(currVal_5)) /* REF:package:corpus_ngdart/src/i92_seguranca_attr_estilo.html:149:171 */;
      this._expr_5 = currVal_5;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I92SegurancaAttrEstilo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I92SegurancaAttrEstiloNgFactory = ComponentFactory<import1.I92SegurancaAttrEstilo>('i92-seguranca-attr-estilo', viewFactory_I92SegurancaAttrEstiloHost0);
ComponentFactory<import1.I92SegurancaAttrEstilo> get I92SegurancaAttrEstiloNgFactory {
  return _I92SegurancaAttrEstiloNgFactory;
}

ComponentFactory<import1.I92SegurancaAttrEstilo> createI92SegurancaAttrEstiloFactory() {
  return ComponentFactory('i92-seguranca-attr-estilo', viewFactory_I92SegurancaAttrEstiloHost0);
}

final List<Object> styles$I92SegurancaAttrEstiloHost = const [];

class _ViewI92SegurancaAttrEstiloHost0 extends import11.HostView<import1.I92SegurancaAttrEstilo> {
  @override
  void build() {
    this.componentView = ViewI92SegurancaAttrEstilo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I92SegurancaAttrEstilo();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I92SegurancaAttrEstilo> viewFactory_I92SegurancaAttrEstiloHost0() {
  return _ViewI92SegurancaAttrEstiloHost0();
}
