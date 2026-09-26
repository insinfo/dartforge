// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i16_view_child_componente.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i16_view_child_componente.dart' as import1;
import 'a02_texto_estatico.template.dart' as import2;
import 'a02_texto_estatico.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I16ViewChildComponente = const [];

class ViewI16ViewChildComponente0 extends import0.ComponentView<import1.I16ViewChildComponente> {
  late final import2.ViewA02TextoEstatico0 _compView_0;
  late final import3.A02TextoEstatico _A02TextoEstatico_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewI16ViewChildComponente0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i16-view-child-componente'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i16_view_child_componente.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA02TextoEstatico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A02TextoEstatico_0_5 = import3.A02TextoEstatico();
    this._compView_0.create(this._A02TextoEstatico_0_5);
    _ctx.filho = this._A02TextoEstatico_0_5;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I16ViewChildComponente, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I16ViewChildComponenteNgFactory = ComponentFactory<import1.I16ViewChildComponente>('i16-view-child-componente', viewFactory_I16ViewChildComponenteHost0);
ComponentFactory<import1.I16ViewChildComponente> get I16ViewChildComponenteNgFactory {
  return _I16ViewChildComponenteNgFactory;
}

ComponentFactory<import1.I16ViewChildComponente> createI16ViewChildComponenteFactory() {
  return ComponentFactory('i16-view-child-componente', viewFactory_I16ViewChildComponenteHost0);
}

final List<Object> styles$I16ViewChildComponenteHost = const [];

class _ViewI16ViewChildComponenteHost0 extends import10.HostView<import1.I16ViewChildComponente> {
  @override
  void build() {
    this.componentView = ViewI16ViewChildComponente0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I16ViewChildComponente();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I16ViewChildComponente> viewFactory_I16ViewChildComponenteHost0() {
  return _ViewI16ViewChildComponenteHost0();
}
