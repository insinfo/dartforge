// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j60_ref_so_em_evento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j60_ref_so_em_evento.dart' as import1;
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

final List<Object> styles$J60RefSoEmEvento = const [];

class ViewJ60RefSoEmEvento0 extends import0.ComponentView<import1.J60RefSoEmEvento> {
  final import2.TextBinding _textBinding_6 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import3.DivElement _el_0;
  late final import3.HtmlElement _el_5;
  late final import3.InputElement _el_8;
  late final import3.SelectElement _el_1;
  static import4.ComponentStyles? _componentStyles;
  ViewJ60RefSoEmEvento0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j60-ref-so-em-evento'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j60_ref_so_em_evento.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendDiv(doc, parentRenderNode);
    this._el_1 = import8.appendElement<import3.SelectElement>(doc, this._el_0, 'select');
    final _el_2 = import8.appendElement<import3.OptionElement>(doc, this._el_1, 'option');
    import8.setAttribute(_el_2, 'value', '1');
    final _text_3 = import8.appendText(_el_2, 'um');
    final _text_4 = import8.appendText(this._el_0, ' ');
    this._el_5 = import8.appendSpan(doc, this._el_0);
    this._el_5.append(this._textBinding_6.element);
    final _text_7 = import8.appendText(this._el_0, ' ');
    this._el_8 = import8.appendElement<import3.InputElement>(doc, this._el_0, 'input');
    this._el_1.addEventListener('change', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.titulo;
    if (import9.checkBinding(this._expr_0, currVal_0, 'titulo', 'package:corpus_ngdart/src/j60_ref_so_em_evento.html')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/j60_ref_so_em_evento.html:5:21 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = (_ctx.ativo == 'a');
    if (import9.checkBinding(this._expr_1, currVal_1, 'ativo == \'a\'', 'package:corpus_ngdart/src/j60_ref_so_em_evento.html')) {
      import8.updateClassBinding(this._el_5, 'ativo', currVal_1) /* REF:package:corpus_ngdart/src/j60_ref_so_em_evento.html:128:156 */;
      this._expr_1 = currVal_1;
    }
    this._textBinding_6.updateText(import10.interpolateString0(_ctx.escolhido)) /* REF:package:corpus_ngdart/src/j60_ref_so_em_evento.html:157:172 */;
    final currVal_2 = _ctx.titulo;
    if (import9.checkBinding(this._expr_2, currVal_2, 'titulo', 'package:corpus_ngdart/src/j60_ref_so_em_evento.html')) {
      import8.setProperty(this._el_8, 'value', currVal_2) /* REF:package:corpus_ngdart/src/j60_ref_so_em_evento.html:189:205 */;
      this._expr_2 = currVal_2;
    }
  }

  void _handleEvent_0($event) {
    final local_campo = this._el_1;
    final _ctx = this.ctx;
    _ctx.escolher(local_campo.value);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J60RefSoEmEvento, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J60RefSoEmEventoNgFactory = ComponentFactory<import1.J60RefSoEmEvento>('j60-ref-so-em-evento', viewFactory_J60RefSoEmEventoHost0);
ComponentFactory<import1.J60RefSoEmEvento> get J60RefSoEmEventoNgFactory {
  return _J60RefSoEmEventoNgFactory;
}

ComponentFactory<import1.J60RefSoEmEvento> createJ60RefSoEmEventoFactory() {
  return ComponentFactory('j60-ref-so-em-evento', viewFactory_J60RefSoEmEventoHost0);
}

final List<Object> styles$J60RefSoEmEventoHost = const [];

class _ViewJ60RefSoEmEventoHost0 extends import12.HostView<import1.J60RefSoEmEvento> {
  @override
  void build() {
    this.componentView = ViewJ60RefSoEmEvento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J60RefSoEmEvento();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J60RefSoEmEvento> viewFactory_J60RefSoEmEventoHost0() {
  return _ViewJ60RefSoEmEventoHost0();
}
