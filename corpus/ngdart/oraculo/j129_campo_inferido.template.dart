// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j129_campo_inferido.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j129_campo_inferido.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/text_binding.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/runtime/interpolate.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;

final List<Object> styles$J129Item = const [];

class ViewJ129Item0 extends import0.ComponentView<import1.J129Item> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ129Item0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j129-item'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j129_campo_inferido.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J129Item, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J129ItemNgFactory = ComponentFactory<import1.J129Item>('j129-item', viewFactory_J129ItemHost0);
ComponentFactory<import1.J129Item> get J129ItemNgFactory {
  return _J129ItemNgFactory;
}

ComponentFactory<import1.J129Item> createJ129ItemFactory() {
  return ComponentFactory('j129-item', viewFactory_J129ItemHost0);
}

final List<Object> styles$J129ItemHost = const [];

class _ViewJ129ItemHost0 extends import8.HostView<import1.J129Item> {
  @override
  void build() {
    this.componentView = ViewJ129Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J129Item();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J129Item> viewFactory_J129ItemHost0() {
  return _ViewJ129ItemHost0();
}

final List<Object> styles$J129Usa = const [];

class ViewJ129Usa0 extends import0.ComponentView<import1.J129Usa> {
  final import9.TextBinding _textBinding_1 = import9.TextBinding();
  final import9.TextBinding _textBinding_3 = import9.TextBinding();
  final import9.TextBinding _textBinding_5 = import9.TextBinding();
  late final ViewJ129Item0 _compView_6;
  late final import1.J129Item _J129Item_6_5;
  Object? _expr_0;
  Object? _expr_1;
  late final import6.HtmlElement _el_6;
  static import2.ComponentStyles? _componentStyles;
  ViewJ129Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j129-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j129_campo_inferido.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import10.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import10.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    final _text_4 = import10.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_5.element);
    this._compView_6 = ViewJ129Item0(this, 6);
    this._el_6 = this._compView_6.rootElement;
    parentRenderNode.append(this._el_6);
    this._J129Item_6_5 = import1.J129Item();
    final _text_7 = import10.createText('x');
    this._compView_6.createAndProject(this._J129Item_6_5, [
      <Object>[_text_7]
    ]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import11.interpolateString0(_ctx.g.papel)) /* REF:asset:corpus_ngdart/lib/src/j129_campo_inferido.dart:491:502 */;
    this._textBinding_3.updateTextWithPrimitive(_ctx.g.quantos) /* REF:asset:corpus_ngdart/lib/src/j129_campo_inferido.dart:503:516 */;
    this._textBinding_5.updateTextWithPrimitive(_ctx.g.ativo) /* REF:asset:corpus_ngdart/lib/src/j129_campo_inferido.dart:517:528 */;
    final currVal_0 = import11.interpolateString0(_ctx.g.papel);
    if (import12.checkBinding(this._expr_0, currVal_0, '{{g.papel}}', 'asset:corpus_ngdart/lib/src/j129_campo_inferido.dart')) {
      import10.setProperty(this._el_6, 'role', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j129_campo_inferido.dart:544:562 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.g.quantos;
    if (import12.checkBinding(this._expr_1, currVal_1, '{{g.quantos}}', 'asset:corpus_ngdart/lib/src/j129_campo_inferido.dart')) {
      import10.setProperty(this._el_6, 'title', import11.interpolate0(currVal_1)) /* REF:asset:corpus_ngdart/lib/src/j129_campo_inferido.dart:563:584 */;
      this._expr_1 = currVal_1;
    }
    this._compView_6.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_6.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J129Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J129UsaNgFactory = ComponentFactory<import1.J129Usa>('j129-usa', viewFactory_J129UsaHost0);
ComponentFactory<import1.J129Usa> get J129UsaNgFactory {
  return _J129UsaNgFactory;
}

ComponentFactory<import1.J129Usa> createJ129UsaFactory() {
  return ComponentFactory('j129-usa', viewFactory_J129UsaHost0);
}

final List<Object> styles$J129UsaHost = const [];

class _ViewJ129UsaHost0 extends import8.HostView<import1.J129Usa> {
  @override
  void build() {
    this.componentView = ViewJ129Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J129Usa();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J129Usa> viewFactory_J129UsaHost0() {
  return _ViewJ129UsaHost0();
}
