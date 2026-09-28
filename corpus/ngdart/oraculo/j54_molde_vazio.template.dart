// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j54_molde_vazio.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j54_molde_vazio.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import12;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import14;

final List<Object> styles$J54MoldeVazio = const [];

class ViewJ54MoldeVazio0 extends import0.ComponentView<import1.J54MoldeVazio> {
  late final ViewContainer _appEl_1;
  late final import1.J54Marca _J54Marca_1_8;
  late final ViewContainer _appEl_2;
  late final TemplateRef _TemplateRef_2_7;
  late final ViewContainer _appEl_4;
  late final import1.J54Marca _J54Marca_4_8;
  static import4.ComponentStyles? _componentStyles;
  ViewJ54MoldeVazio0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j54-molde-vazio'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j54_molde_vazio.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_7 = TemplateRef(this._appEl_1, viewFactory_J54MoldeVazio1);
    this._J54Marca_1_8 = import1.J54Marca(_TemplateRef_1_7);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_1, this._J54Marca_1_8);
    }
    final _anchor_2 = import9.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    this._TemplateRef_2_7 = TemplateRef(this._appEl_2, viewFactory_J54MoldeVazio2);
    final _text_3 = import9.appendText(parentRenderNode, '\n');
    final _anchor_4 = import9.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_7 = TemplateRef(this._appEl_4, viewFactory_J54MoldeVazio3);
    this._J54Marca_4_8 = import1.J54Marca(_TemplateRef_4_7);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_4, this._J54Marca_4_8);
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J54MoldeVazio, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J54MoldeVazioNgFactory = ComponentFactory<import1.J54MoldeVazio>('j54-molde-vazio', viewFactory_J54MoldeVazioHost0);
ComponentFactory<import1.J54MoldeVazio> get J54MoldeVazioNgFactory {
  return _J54MoldeVazioNgFactory;
}

ComponentFactory<import1.J54MoldeVazio> createJ54MoldeVazioFactory() {
  return ComponentFactory('j54-molde-vazio', viewFactory_J54MoldeVazioHost0);
}

class _ViewJ54MoldeVazio1 extends import12.EmbeddedView<import1.J54MoldeVazio> {
  _ViewJ54MoldeVazio1(import13.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import12.EmbeddedView<void> viewFactory_J54MoldeVazio1(import13.RenderView parentView, int parentIndex) {
  return _ViewJ54MoldeVazio1(parentView, parentIndex);
}

class _ViewJ54MoldeVazio2 extends import12.EmbeddedView<import1.J54MoldeVazio> {
  _ViewJ54MoldeVazio2(import13.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import12.EmbeddedView<void> viewFactory_J54MoldeVazio2(import13.RenderView parentView, int parentIndex) {
  return _ViewJ54MoldeVazio2(parentView, parentIndex);
}

class _ViewJ54MoldeVazio3 extends import12.EmbeddedView<import1.J54MoldeVazio> {
  _ViewJ54MoldeVazio3(import13.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import12.EmbeddedView<void> viewFactory_J54MoldeVazio3(import13.RenderView parentView, int parentIndex) {
  return _ViewJ54MoldeVazio3(parentView, parentIndex);
}

final List<Object> styles$J54MoldeVazioHost = const [];

class _ViewJ54MoldeVazioHost0 extends import14.HostView<import1.J54MoldeVazio> {
  @override
  void build() {
    this.componentView = ViewJ54MoldeVazio0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J54MoldeVazio();
    this.initRootNode(_el_0);
  }
}

import14.HostView<import1.J54MoldeVazio> viewFactory_J54MoldeVazioHost0() {
  return _ViewJ54MoldeVazioHost0();
}
