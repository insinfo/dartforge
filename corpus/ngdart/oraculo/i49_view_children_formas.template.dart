// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i49_view_children_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i49_view_children_formas.dart' as import1;
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

final List<Object> styles$I49ViewChildrenFormas = const [];

class ViewI49ViewChildrenFormas0 extends import0.ComponentView<import1.I49ViewChildrenFormas> {
  late final import2.ViewA02TextoEstatico0 _compView_0;
  late final import3.A02TextoEstatico _A02TextoEstatico_0_5;
  late final import2.ViewA02TextoEstatico0 _compView_3;
  late final import3.A02TextoEstatico _A02TextoEstatico_3_5;
  static import4.ComponentStyles? _componentStyles;
  ViewI49ViewChildrenFormas0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i49-view-children-formas'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i49_view_children_formas.dart' : null);
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
    final doc = import8.document;
    final _el_1 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_2 = import9.appendText(_el_1, '1');
    this._compView_3 = import2.ViewA02TextoEstatico0(this, 3);
    final _el_3 = this._compView_3.rootElement;
    parentRenderNode.append(_el_3);
    this._A02TextoEstatico_3_5 = import3.A02TextoEstatico();
    this._compView_3.create(this._A02TextoEstatico_3_5);
    final _el_4 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_5 = import9.appendText(_el_4, '2');
    _ctx.filhos = [this._A02TextoEstatico_0_5, this._A02TextoEstatico_3_5];
    _ctx.primeiro = _el_1;
    _ctx.nenhum = [];
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_3.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_3.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I49ViewChildrenFormas, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I49ViewChildrenFormasNgFactory = ComponentFactory<import1.I49ViewChildrenFormas>('i49-view-children-formas', viewFactory_I49ViewChildrenFormasHost0);
ComponentFactory<import1.I49ViewChildrenFormas> get I49ViewChildrenFormasNgFactory {
  return _I49ViewChildrenFormasNgFactory;
}

ComponentFactory<import1.I49ViewChildrenFormas> createI49ViewChildrenFormasFactory() {
  return ComponentFactory('i49-view-children-formas', viewFactory_I49ViewChildrenFormasHost0);
}

final List<Object> styles$I49ViewChildrenFormasHost = const [];

class _ViewI49ViewChildrenFormasHost0 extends import11.HostView<import1.I49ViewChildrenFormas> {
  @override
  void build() {
    this.componentView = ViewI49ViewChildrenFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I49ViewChildrenFormas();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I49ViewChildrenFormas> viewFactory_I49ViewChildrenFormasHost0() {
  return _ViewI49ViewChildrenFormasHost0();
}
