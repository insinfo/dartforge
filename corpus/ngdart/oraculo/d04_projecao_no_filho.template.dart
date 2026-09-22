// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd04_projecao_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd04_projecao_no_filho.dart' as import1;
import 'a11_projecao.template.dart' as import2;
import 'a11_projecao.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$D04ProjecaoNoFilho = const [];

class ViewD04ProjecaoNoFilho0 extends import0.ComponentView<import1.D04ProjecaoNoFilho> {
  late final import2.ViewA11Projecao0 _compView_0;
  late final import3.A11Projecao _A11Projecao_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewD04ProjecaoNoFilho0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d04-projecao-no-filho'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d04_projecao_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA11Projecao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A11Projecao_0_5 = import3.A11Projecao();
    final doc = import8.document;
    final _el_1 = import7.unsafeCast(doc.createElement('span'));
    final _text_2 = import9.appendText(_el_1, 'dentro');
    this._compView_0.createAndProject(this._A11Projecao_0_5, [
      <Object>[_el_1]
    ]);
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D04ProjecaoNoFilho, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D04ProjecaoNoFilhoNgFactory = ComponentFactory<import1.D04ProjecaoNoFilho>('d04-projecao-no-filho', viewFactory_D04ProjecaoNoFilhoHost0);
ComponentFactory<import1.D04ProjecaoNoFilho> get D04ProjecaoNoFilhoNgFactory {
  return _D04ProjecaoNoFilhoNgFactory;
}

ComponentFactory<import1.D04ProjecaoNoFilho> createD04ProjecaoNoFilhoFactory() {
  return ComponentFactory('d04-projecao-no-filho', viewFactory_D04ProjecaoNoFilhoHost0);
}

final List<Object> styles$D04ProjecaoNoFilhoHost = const [];

class _ViewD04ProjecaoNoFilhoHost0 extends import11.HostView<import1.D04ProjecaoNoFilho> {
  @override
  void build() {
    this.componentView = ViewD04ProjecaoNoFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D04ProjecaoNoFilho();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.D04ProjecaoNoFilho> viewFactory_D04ProjecaoNoFilhoHost0() {
  return _ViewD04ProjecaoNoFilhoHost0();
}
