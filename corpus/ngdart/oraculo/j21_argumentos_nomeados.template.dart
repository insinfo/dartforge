// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j21_argumentos_nomeados.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j21_argumentos_nomeados.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J21ArgumentosNomeados = const [];

class ViewJ21ArgumentosNomeados0 extends import0.ComponentView<import1.J21ArgumentosNomeados> {
  final import2.TextBinding _textBinding_10 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ21ArgumentosNomeados0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j21-argumentos-nomeados'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j21_argumentos_nomeados.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    final _el_0 = import8.appendElement<import3.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_1 = import8.appendText(_el_0, 'a');
    final _text_2 = import8.appendText(parentRenderNode, '\n');
    final _el_3 = import8.appendElement<import3.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_4 = import8.appendText(_el_3, 'b');
    final _text_5 = import8.appendText(parentRenderNode, '\n');
    final _el_6 = import8.appendElement<import3.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_7 = import8.appendText(_el_6, 'c');
    final _text_8 = import8.appendText(parentRenderNode, '\n');
    this._el_9 = import8.appendSpan(doc, parentRenderNode);
    this._el_9.append(this._textBinding_10.element);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    _el_3.addEventListener('click', this.eventHandler1(this._handleEvent_1));
    _el_6.addEventListener('click', this.eventHandler1(this._handleEvent_2));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.rotulo(
      curto: true,
    );
    if (import9.checkBinding(this._expr_0, currVal_0, 'rotulo(curto: true)', 'package:corpus_ngdart/src/j21_argumentos_nomeados.html')) {
      import8.setProperty(this._el_9, 'title', currVal_0) /* REF:package:corpus_ngdart/src/j21_argumentos_nomeados.html:167:196 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_10.updateText(import10.interpolateString0(_ctx.rotulo(
          curto: false,
        ))) /* REF:package:corpus_ngdart/src/j21_argumentos_nomeados.html:197:221 */;
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.fechar(
      origem: 'fundo',
    );
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.mover(
      1,
      b: true,
    );
  }

  void _handleEvent_2($event) {
    final _ctx = this.ctx;
    _ctx.aninhar(
      v: _ctx.rotulo(
        curto: true,
      ),
    );
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J21ArgumentosNomeados, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J21ArgumentosNomeadosNgFactory = ComponentFactory<import1.J21ArgumentosNomeados>('j21-argumentos-nomeados', viewFactory_J21ArgumentosNomeadosHost0);
ComponentFactory<import1.J21ArgumentosNomeados> get J21ArgumentosNomeadosNgFactory {
  return _J21ArgumentosNomeadosNgFactory;
}

ComponentFactory<import1.J21ArgumentosNomeados> createJ21ArgumentosNomeadosFactory() {
  return ComponentFactory('j21-argumentos-nomeados', viewFactory_J21ArgumentosNomeadosHost0);
}

final List<Object> styles$J21ArgumentosNomeadosHost = const [];

class _ViewJ21ArgumentosNomeadosHost0 extends import12.HostView<import1.J21ArgumentosNomeados> {
  @override
  void build() {
    this.componentView = ViewJ21ArgumentosNomeados0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J21ArgumentosNomeados();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J21ArgumentosNomeados> viewFactory_J21ArgumentosNomeadosHost0() {
  return _ViewJ21ArgumentosNomeadosHost0();
}
