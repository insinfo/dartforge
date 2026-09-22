// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd01_dois_filhos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd01_dois_filhos.dart' as import1;
import 'a02_texto_estatico.template.dart' as import2;
import 'a02_texto_estatico.dart' as import3;
import 'a11_projecao.template.dart' as import4;
import 'a11_projecao.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$D01DoisFilhos = const [];

class ViewD01DoisFilhos0 extends import0.ComponentView<import1.D01DoisFilhos> {
  late final import2.ViewA02TextoEstatico0 _compView_0;
  late final import3.A02TextoEstatico _A02TextoEstatico_0_5;
  late final import4.ViewA11Projecao0 _compView_1;
  late final import5.A11Projecao _A11Projecao_1_5;
  static import6.ComponentStyles? _componentStyles;
  ViewD01DoisFilhos0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('d01-dois-filhos'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/d01_dois_filhos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA02TextoEstatico0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A02TextoEstatico_0_5 = import3.A02TextoEstatico();
    this._compView_0.create(this._A02TextoEstatico_0_5);
    this._compView_1 = import4.ViewA11Projecao0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._A11Projecao_1_5 = import5.A11Projecao();
    this._compView_1.createAndProject(this._A11Projecao_1_5, [const <Object>[]]);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$D01DoisFilhos, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D01DoisFilhosNgFactory = ComponentFactory<import1.D01DoisFilhos>('d01-dois-filhos', viewFactory_D01DoisFilhosHost0);
ComponentFactory<import1.D01DoisFilhos> get D01DoisFilhosNgFactory {
  return _D01DoisFilhosNgFactory;
}

ComponentFactory<import1.D01DoisFilhos> createD01DoisFilhosFactory() {
  return ComponentFactory('d01-dois-filhos', viewFactory_D01DoisFilhosHost0);
}

final List<Object> styles$D01DoisFilhosHost = const [];

class _ViewD01DoisFilhosHost0 extends import12.HostView<import1.D01DoisFilhos> {
  @override
  void build() {
    this.componentView = ViewD01DoisFilhos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D01DoisFilhos();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.D01DoisFilhos> viewFactory_D01DoisFilhosHost0() {
  return _ViewD01DoisFilhosHost0();
}
