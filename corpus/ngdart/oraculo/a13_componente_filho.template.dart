// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a13_componente_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a13_componente_filho.dart' as import1;
import 'a02_texto_estatico.template.dart' as import2;
import 'a02_texto_estatico.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$A13ComponenteFilho = const [];

class ViewA13ComponenteFilho0 extends import0.ComponentView<import1.A13ComponenteFilho> {
  late final import2.ViewA02TextoEstatico0 _compView_0;
  late final import3.A02TextoEstatico _A02TextoEstatico_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewA13ComponenteFilho0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('a13-componente-filho'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a13_componente_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA02TextoEstatico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A02TextoEstatico_0_5 = import3.A02TextoEstatico();
    this._compView_0.create(this._A02TextoEstatico_0_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A13ComponenteFilho, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A13ComponenteFilhoNgFactory = ComponentFactory<import1.A13ComponenteFilho>('a13-componente-filho', viewFactory_A13ComponenteFilhoHost0);
ComponentFactory<import1.A13ComponenteFilho> get A13ComponenteFilhoNgFactory {
  return _A13ComponenteFilhoNgFactory;
}

ComponentFactory<import1.A13ComponenteFilho> createA13ComponenteFilhoFactory() {
  return ComponentFactory('a13-componente-filho', viewFactory_A13ComponenteFilhoHost0);
}

final List<Object> styles$A13ComponenteFilhoHost = const [];

class _ViewA13ComponenteFilhoHost0 extends import10.HostView<import1.A13ComponenteFilho> {
  @override
  void build() {
    this.componentView = ViewA13ComponenteFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A13ComponenteFilho();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.A13ComponenteFilho> viewFactory_A13ComponenteFilhoHost0() {
  return _ViewA13ComponenteFilhoHost0();
}
