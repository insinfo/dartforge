// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i50_view_child_tipos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i50_view_child_tipos.dart' as import1;
import 'a11_projecao.template.dart' as import2;
import 'a11_projecao.dart' as import3;
import 'a02_texto_estatico.template.dart' as import4;
import 'a02_texto_estatico.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I50ViewChildTipos = const [];

class ViewI50ViewChildTipos0 extends import0.ComponentView<import1.I50ViewChildTipos> {
  late final import2.ViewA11Projecao0 _compView_0;
  late final import3.A11Projecao _A11Projecao_0_5;
  late final import4.ViewA02TextoEstatico0 _compView_1;
  late final import5.A02TextoEstatico _A02TextoEstatico_1_5;
  late final import4.ViewA02TextoEstatico0 _compView_2;
  late final import5.A02TextoEstatico _A02TextoEstatico_2_5;
  static import6.ComponentStyles? _componentStyles;
  ViewI50ViewChildTipos0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i50-view-child-tipos'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i50_view_child_tipos.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA11Projecao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A11Projecao_0_5 = import3.A11Projecao();
    this._compView_1 = import4.ViewA02TextoEstatico0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    this._A02TextoEstatico_1_5 = import5.A02TextoEstatico();
    this._compView_1.create(this._A02TextoEstatico_1_5);
    this._compView_0.createAndProject(this._A11Projecao_0_5, [
      <Object>[_el_1]
    ]);
    this._compView_2 = import4.ViewA02TextoEstatico0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._A02TextoEstatico_2_5 = import5.A02TextoEstatico();
    this._compView_2.create(this._A02TextoEstatico_2_5);
    _ctx.todos = [this._A02TextoEstatico_1_5, this._A02TextoEstatico_2_5];
    _ctx.primeiro = this._A02TextoEstatico_1_5;
    _ctx.projecoes = [this._A11Projecao_0_5];
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I50ViewChildTipos, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I50ViewChildTiposNgFactory = ComponentFactory<import1.I50ViewChildTipos>('i50-view-child-tipos', viewFactory_I50ViewChildTiposHost0);
ComponentFactory<import1.I50ViewChildTipos> get I50ViewChildTiposNgFactory {
  return _I50ViewChildTiposNgFactory;
}

ComponentFactory<import1.I50ViewChildTipos> createI50ViewChildTiposFactory() {
  return ComponentFactory('i50-view-child-tipos', viewFactory_I50ViewChildTiposHost0);
}

final List<Object> styles$I50ViewChildTiposHost = const [];

class _ViewI50ViewChildTiposHost0 extends import12.HostView<import1.I50ViewChildTipos> {
  @override
  void build() {
    this.componentView = ViewI50ViewChildTipos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I50ViewChildTipos();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I50ViewChildTipos> viewFactory_I50ViewChildTiposHost0() {
  return _ViewI50ViewChildTiposHost0();
}
