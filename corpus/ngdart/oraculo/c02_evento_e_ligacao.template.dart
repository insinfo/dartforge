// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c02_evento_e_ligacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c02_evento_e_ligacao.dart' as import1;
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

final List<Object> styles$C02EventoELigacao = const [];

class ViewC02EventoELigacao0 extends import0.ComponentView<import1.C02EventoELigacao> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.ButtonElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewC02EventoELigacao0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('c02-evento-e-ligacao'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/c02_evento_e_ligacao.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.ButtonElement>(doc, parentRenderNode, 'button');
    this._el_0.append(this._textBinding_1.element);
    this._el_0.addEventListener('click', this.eventHandler0(_ctx.fazer));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.travado;
    if (import9.checkBinding(this._expr_0, currVal_0, 'travado', 'package:corpus_ngdart/src/c02_evento_e_ligacao.html')) {
      import8.setProperty(this._el_0, 'disabled', currVal_0) /* REF:package:corpus_ngdart/src/c02_evento_e_ligacao.html:26:46 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.rotulo)) /* REF:package:corpus_ngdart/src/c02_evento_e_ligacao.html:47:57 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$C02EventoELigacao, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C02EventoELigacaoNgFactory = ComponentFactory<import1.C02EventoELigacao>('c02-evento-e-ligacao', viewFactory_C02EventoELigacaoHost0);
ComponentFactory<import1.C02EventoELigacao> get C02EventoELigacaoNgFactory {
  return _C02EventoELigacaoNgFactory;
}

ComponentFactory<import1.C02EventoELigacao> createC02EventoELigacaoFactory() {
  return ComponentFactory('c02-evento-e-ligacao', viewFactory_C02EventoELigacaoHost0);
}

final List<Object> styles$C02EventoELigacaoHost = const [];

class _ViewC02EventoELigacaoHost0 extends import12.HostView<import1.C02EventoELigacao> {
  @override
  void build() {
    this.componentView = ViewC02EventoELigacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C02EventoELigacao();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.C02EventoELigacao> viewFactory_C02EventoELigacaoHost0() {
  return _ViewC02EventoELigacaoHost0();
}
