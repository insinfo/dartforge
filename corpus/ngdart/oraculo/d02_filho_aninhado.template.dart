// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd02_filho_aninhado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd02_filho_aninhado.dart' as import1;
import 'a02_texto_estatico.template.dart' as import2;
import 'a02_texto_estatico.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$D02FilhoAninhado = const [];

class ViewD02FilhoAninhado0 extends import0.ComponentView<import1.D02FilhoAninhado> {
  late final import2.ViewA02TextoEstatico0 _compView_3;
  late final import3.A02TextoEstatico _A02TextoEstatico_3_5;
  static import4.ComponentStyles? _componentStyles;
  ViewD02FilhoAninhado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d02-filho-aninhado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d02_filho_aninhado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    final _el_1 = import9.appendSpan(doc, _el_0);
    final _text_2 = import9.appendText(_el_1, 'a');
    this._compView_3 = import2.ViewA02TextoEstatico0(this, 3);
    final _el_3 = this._compView_3.rootElement;
    _el_0.append(_el_3);
    this._A02TextoEstatico_3_5 = import3.A02TextoEstatico();
    this._compView_3.create(this._A02TextoEstatico_3_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_3.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_3.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D02FilhoAninhado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D02FilhoAninhadoNgFactory = ComponentFactory<import1.D02FilhoAninhado>('d02-filho-aninhado', viewFactory_D02FilhoAninhadoHost0);
ComponentFactory<import1.D02FilhoAninhado> get D02FilhoAninhadoNgFactory {
  return _D02FilhoAninhadoNgFactory;
}

ComponentFactory<import1.D02FilhoAninhado> createD02FilhoAninhadoFactory() {
  return ComponentFactory('d02-filho-aninhado', viewFactory_D02FilhoAninhadoHost0);
}

final List<Object> styles$D02FilhoAninhadoHost = const [];

class _ViewD02FilhoAninhadoHost0 extends import11.HostView<import1.D02FilhoAninhado> {
  @override
  void build() {
    this.componentView = ViewD02FilhoAninhado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D02FilhoAninhado();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.D02FilhoAninhado> viewFactory_D02FilhoAninhadoHost0() {
  return _ViewD02FilhoAninhadoHost0();
}
