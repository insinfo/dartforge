// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i06_ref_em_evento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i06_ref_em_evento.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I06RefEmEvento = const [];

class ViewI06RefEmEvento0 extends import0.ComponentView<import1.I06RefEmEvento> {
  late final import2.InputElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI06RefEmEvento0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i06-ref-em-evento'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i06_ref_em_evento.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.InputElement>(doc, parentRenderNode, 'input');
    final _el_1 = import7.appendElement<import2.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_2 = import7.appendText(_el_1, '+');
    _el_1.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  void _handleEvent_0($event) {
    final local_campo = this._el_0;
    final _ctx = this.ctx;
    _ctx.adicionar(local_campo.value);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I06RefEmEvento, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I06RefEmEventoNgFactory = ComponentFactory<import1.I06RefEmEvento>('i06-ref-em-evento', viewFactory_I06RefEmEventoHost0);
ComponentFactory<import1.I06RefEmEvento> get I06RefEmEventoNgFactory {
  return _I06RefEmEventoNgFactory;
}

ComponentFactory<import1.I06RefEmEvento> createI06RefEmEventoFactory() {
  return ComponentFactory('i06-ref-em-evento', viewFactory_I06RefEmEventoHost0);
}

final List<Object> styles$I06RefEmEventoHost = const [];

class _ViewI06RefEmEventoHost0 extends import9.HostView<import1.I06RefEmEvento> {
  @override
  void build() {
    this.componentView = ViewI06RefEmEvento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I06RefEmEvento();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.I06RefEmEvento> viewFactory_I06RefEmEventoHost0() {
  return _ViewI06RefEmEventoHost0();
}
