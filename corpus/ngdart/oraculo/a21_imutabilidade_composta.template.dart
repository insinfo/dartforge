// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a21_imutabilidade_composta.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a21_imutabilidade_composta.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$A21ImutabilidadeComposta = const [];

class ViewA21ImutabilidadeComposta0 extends import0.ComponentView<import1.A21ImutabilidadeComposta> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_8 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  late final import3.HtmlElement _el_2;
  late final import3.DivElement _el_3;
  late final import3.AnchorElement _el_4;
  static import4.ComponentStyles? _componentStyles;
  ViewA21ImutabilidadeComposta0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('a21-imutabilidade-composta'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a21_imutabilidade_composta.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    final _el_0 = import8.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    this._el_2 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_3 = import8.appendDiv(doc, parentRenderNode);
    this._el_4 = import8.appendElement<import3.AnchorElement>(doc, parentRenderNode, 'a');
    final _el_5 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_6 = import8.appendText(_el_5, import9.interpolate0((_ctx.numero ?? 1)));
    final _el_7 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'i');
    _el_7.append(this._textBinding_8.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.item.nome)) /* REF:package:corpus_ngdart/src/a21_imutabilidade_composta.html:6:19 */;
    final currVal_0 = _ctx.item.nome;
    if (import10.checkBinding(this._expr_0, currVal_0, 'item.nome', 'package:corpus_ngdart/src/a21_imutabilidade_composta.html')) {
      import8.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/a21_imutabilidade_composta.html:29:48 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = (!_ctx.fixo);
    if (import10.checkBinding(this._expr_1, currVal_1, '!fixo', 'package:corpus_ngdart/src/a21_imutabilidade_composta.html')) {
      import8.setProperty(this._el_3, 'hidden', currVal_1) /* REF:package:corpus_ngdart/src/a21_imutabilidade_composta.html:58:74 */;
      this._expr_1 = currVal_1;
    }
    if (firstCheck) {
      if ((_ctx.rotulo != null)) {
        import8.setProperty(this._el_4, 'title', _ctx.rotulo) /* REF:package:corpus_ngdart/src/a21_imutabilidade_composta.html:84:100 */;
      }
    }
    this._textBinding_8.updateText(import9.interpolate0((_ctx.talvez ?? 'x'))) /* REF:package:corpus_ngdart/src/a21_imutabilidade_composta.html:130:147 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A21ImutabilidadeComposta, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A21ImutabilidadeCompostaNgFactory = ComponentFactory<import1.A21ImutabilidadeComposta>('a21-imutabilidade-composta', viewFactory_A21ImutabilidadeCompostaHost0);
ComponentFactory<import1.A21ImutabilidadeComposta> get A21ImutabilidadeCompostaNgFactory {
  return _A21ImutabilidadeCompostaNgFactory;
}

ComponentFactory<import1.A21ImutabilidadeComposta> createA21ImutabilidadeCompostaFactory() {
  return ComponentFactory('a21-imutabilidade-composta', viewFactory_A21ImutabilidadeCompostaHost0);
}

final List<Object> styles$A21ImutabilidadeCompostaHost = const [];

class _ViewA21ImutabilidadeCompostaHost0 extends import12.HostView<import1.A21ImutabilidadeComposta> {
  @override
  void build() {
    this.componentView = ViewA21ImutabilidadeComposta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A21ImutabilidadeComposta();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.A21ImutabilidadeComposta> viewFactory_A21ImutabilidadeCompostaHost0() {
  return _ViewA21ImutabilidadeCompostaHost0();
}
