// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j57_reprojecao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j57_reprojecao.dart' as import1;
import 'j41_injecao_no_conteudo.template.dart' as import2;
import 'j41_injecao_no_conteudo.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J57Reprojecao = const [];

class ViewJ57Reprojecao0 extends import0.ComponentView<import1.J57Reprojecao> {
  late final import2.ViewJ41Aba0 _compView_0;
  late final import3.J41Aba _J41Aba_0_5;
  late final import2.ViewJ41Aba0 _compView_1;
  late final import3.J41Aba _J41Aba_1_5;
  static import4.ComponentStyles? _componentStyles;
  ViewJ57Reprojecao0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j57-reprojecao'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j57_reprojecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ41Aba0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J41Aba_0_5 = import3.J41Aba();
    this._compView_0.createAndProject(this._J41Aba_0_5, [this.projectedNodes[0]]);
    this._compView_1 = import2.ViewJ41Aba0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J41Aba_1_5 = import3.J41Aba();
    final doc = import8.document;
    final _el_2 = import7.unsafeCast(doc.createElement('b'));
    final _text_3 = import9.appendText(_el_2, 'x');
    this._compView_1.createAndProject(this._J41Aba_1_5, [
      <Object>[_el_2]..addAll(import7.unsafeCast(this.projectedNodes[1]))
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import3.J41Aba) && (0 == nodeIndex))) {
      return this._J41Aba_0_5;
    }
    if ((identical(token, import3.J41Aba) && ((1 <= nodeIndex) && (nodeIndex <= 3)))) {
      return this._J41Aba_1_5;
    }
    return notFoundResult;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J57Reprojecao, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J57ReprojecaoNgFactory = ComponentFactory<import1.J57Reprojecao>('j57-reprojecao', viewFactory_J57ReprojecaoHost0);
ComponentFactory<import1.J57Reprojecao> get J57ReprojecaoNgFactory {
  return _J57ReprojecaoNgFactory;
}

ComponentFactory<import1.J57Reprojecao> createJ57ReprojecaoFactory() {
  return ComponentFactory('j57-reprojecao', viewFactory_J57ReprojecaoHost0);
}

final List<Object> styles$J57ReprojecaoHost = const [];

class _ViewJ57ReprojecaoHost0 extends import11.HostView<import1.J57Reprojecao> {
  @override
  void build() {
    this.componentView = ViewJ57Reprojecao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J57Reprojecao();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J57Reprojecao> viewFactory_J57ReprojecaoHost0() {
  return _ViewJ57ReprojecaoHost0();
}
