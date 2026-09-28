// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j48_usa_container.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j48_usa_container.dart' as import1;
import 'j47_container_no_hospedeiro.template.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'j47_container_no_hospedeiro.dart' as import4;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'j41_injecao_no_conteudo.template.dart' as import6;
import 'j41_injecao_no_conteudo.dart' as import7;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import8;
import 'package:ngdart/src/core/linker/views/view.dart' as import9;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import10;
import 'package:ngdart/src/utilities.dart' as import11;
import 'dart:html' as import12;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import13;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J48UsaContainer = const [];

class ViewJ48UsaContainer0 extends import0.ComponentView<import1.J48UsaContainer> {
  late final import2.ViewJ47Simples0 _compView_0;
  late final ViewContainer _appEl_0;
  late final import4.J47Simples _J47Simples_0_8;
  late final import2.ViewJ47Simples0 _compView_2;
  late final ViewContainer _appEl_2;
  late final import4.J47Simples _J47Simples_2_8;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final import6.ViewJ41Aba0 _compView_5;
  late final import7.J41Aba _J41Aba_5_5;
  late final import2.ViewJ47Simples0 _compView_6;
  late final ViewContainer _appEl_6;
  late final import4.J47Simples _J47Simples_6_8;
  static import8.ComponentStyles? _componentStyles;
  ViewJ48UsaContainer0(import9.View parentView, int parentIndex) : super(parentView, parentIndex, import10.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import11.unsafeCast(import12.document.createElement('j48-usa-container'));
  }
  static String? get _debugComponentUrl {
    return (import11.isDevMode ? 'asset:corpus_ngdart/lib/src/j48_usa_container.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ47Simples0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J47Simples_0_8 = import4.J47Simples(this._appEl_0);
    this._compView_0.create(this._J47Simples_0_8);
    final doc = import12.document;
    final _el_1 = import13.appendDiv(doc, parentRenderNode);
    this._compView_2 = import2.ViewJ47Simples0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    _el_1.append(_el_2);
    this._appEl_2 = ViewContainer(2, 1, this, _el_2);
    this._J47Simples_2_8 = import4.J47Simples(this._appEl_2);
    this._compView_2.create(this._J47Simples_2_8);
    final _anchor_3 = import13.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J48UsaContainer1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    final _anchor_4 = import13.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J48UsaContainer2);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    this._compView_5 = import6.ViewJ41Aba0(this, 5);
    final _el_5 = this._compView_5.rootElement;
    parentRenderNode.append(_el_5);
    this._J41Aba_5_5 = import7.J41Aba();
    this._compView_6 = import2.ViewJ47Simples0(this, 6);
    final _el_6 = this._compView_6.rootElement;
    this._appEl_6 = ViewContainer(6, 5, this, _el_6);
    this._J47Simples_6_8 = import4.J47Simples(this._appEl_6);
    this._compView_6.create(this._J47Simples_6_8);
    this._compView_5.createAndProject(this._J41Aba_5_5, [
      <Object>[this._appEl_6]
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import7.J41Aba) && ((5 <= nodeIndex) && (nodeIndex <= 6)))) {
      return this._J41Aba_5_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_3_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j48_usa_container.html:70:85 */;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_4_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j48_usa_container.html:131:146 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
    this._appEl_4.detectChangesInNestedViews();
    this._appEl_6.detectChangesInNestedViews();
    this._compView_0.detectChanges();
    this._compView_2.detectChanges();
    this._compView_5.detectChanges();
    this._compView_6.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
    this._appEl_4.destroyNestedViews();
    this._appEl_6.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_2.destroyInternalState();
    this._compView_5.destroyInternalState();
    this._compView_6.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import8.ComponentStyles.unscoped(styles$J48UsaContainer, _debugComponentUrl));
      if (import11.isDevMode) {
        import8.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J48UsaContainerNgFactory = ComponentFactory<import1.J48UsaContainer>('j48-usa-container', viewFactory_J48UsaContainerHost0);
ComponentFactory<import1.J48UsaContainer> get J48UsaContainerNgFactory {
  return _J48UsaContainerNgFactory;
}

ComponentFactory<import1.J48UsaContainer> createJ48UsaContainerFactory() {
  return ComponentFactory('j48-usa-container', viewFactory_J48UsaContainerHost0);
}

class _ViewJ48UsaContainer1 extends import17.EmbeddedView<import1.J48UsaContainer> {
  late final import2.ViewJ47Simples0 _compView_1;
  late final ViewContainer _appEl_1;
  late final import4.J47Simples _J47Simples_1_8;
  _ViewJ48UsaContainer1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import12.document;
    final _el_0 = import11.unsafeCast(doc.createElement('p'));
    this._compView_1 = import2.ViewJ47Simples0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._appEl_1 = ViewContainer(1, 0, this, _el_1);
    this._J47Simples_1_8 = import4.J47Simples(this._appEl_1);
    this._compView_1.create(this._J47Simples_1_8);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._appEl_1.detectChangesInNestedViews();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._compView_1.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_J48UsaContainer1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ48UsaContainer1(parentView, parentIndex);
}

class _ViewJ48UsaContainer2 extends import17.EmbeddedView<import1.J48UsaContainer> {
  late final import2.ViewJ47Simples0 _compView_0;
  late final ViewContainer _appEl_0;
  late final import4.J47Simples _J47Simples_0_8;
  _ViewJ48UsaContainer2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = import2.ViewJ47Simples0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._appEl_0 = ViewContainer(0, null, this, _el_0);
    this._J47Simples_0_8 = import4.J47Simples(this._appEl_0);
    this._compView_0.create(this._J47Simples_0_8);
    this.initRootNode(this._appEl_0);
  }

  @override
  void detectChangesInternal() {
    this._appEl_0.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_J48UsaContainer2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ48UsaContainer2(parentView, parentIndex);
}

final List<Object> styles$J48UsaContainerHost = const [];

class _ViewJ48UsaContainerHost0 extends import19.HostView<import1.J48UsaContainer> {
  @override
  void build() {
    this.componentView = ViewJ48UsaContainer0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J48UsaContainer();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J48UsaContainer> viewFactory_J48UsaContainerHost0() {
  return _ViewJ48UsaContainerHost0();
}
