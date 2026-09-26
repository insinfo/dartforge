// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i24_two_way_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i24_two_way_filho.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;

final List<Object> styles$I24Contador = const [];

class ViewI24Contador0 extends import0.ComponentView<import1.I24Contador> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewI24Contador0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i24-contador'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i24_two_way_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateTextWithPrimitive(_ctx.valor) /* REF:asset:corpus_ngdart/lib/src/i24_two_way_filho.dart:112:121 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I24Contador, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I24ContadorNgFactory = ComponentFactory<import1.I24Contador>('i24-contador', viewFactory_I24ContadorHost0);
ComponentFactory<import1.I24Contador> get I24ContadorNgFactory {
  return _I24ContadorNgFactory;
}

ComponentFactory<import1.I24Contador> createI24ContadorFactory() {
  return ComponentFactory('i24-contador', viewFactory_I24ContadorHost0);
}

final List<Object> styles$I24ContadorHost = const [];

class _ViewI24ContadorHost0 extends import10.HostView<import1.I24Contador> {
  @override
  void build() {
    this.componentView = ViewI24Contador0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I24Contador();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I24Contador> viewFactory_I24ContadorHost0() {
  return _ViewI24ContadorHost0();
}

final List<Object> styles$I24TwoWayFilho = const [];

class ViewI24TwoWayFilho0 extends import0.ComponentView<import1.I24TwoWayFilho> {
  late final ViewI24Contador0 _compView_0;
  late final import1.I24Contador _I24Contador_0_5;
  Object? _expr_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI24TwoWayFilho0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i24-two-way-filho'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i24_two_way_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewI24Contador0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I24Contador_0_5 = import1.I24Contador();
    this._compView_0.create(this._I24Contador_0_5);
    final subscription_0 = this._I24Contador_0_5.valorChange.listen(this.eventHandler1(this._handleEvent_0));
    this.initSubscriptions([subscription_0]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.n;
    if (import11.checkBinding(this._expr_0, currVal_0, 'n', 'package:corpus_ngdart/src/i24_two_way_filho.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._I24Contador_0_5, 'valor', currVal_0);
      }
      this._I24Contador_0_5.valor = currVal_0 /* REF:package:corpus_ngdart/src/i24_two_way_filho.html:14:27 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.n = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I24TwoWayFilho, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I24TwoWayFilhoNgFactory = ComponentFactory<import1.I24TwoWayFilho>('i24-two-way-filho', viewFactory_I24TwoWayFilhoHost0);
ComponentFactory<import1.I24TwoWayFilho> get I24TwoWayFilhoNgFactory {
  return _I24TwoWayFilhoNgFactory;
}

ComponentFactory<import1.I24TwoWayFilho> createI24TwoWayFilhoFactory() {
  return ComponentFactory('i24-two-way-filho', viewFactory_I24TwoWayFilhoHost0);
}

final List<Object> styles$I24TwoWayFilhoHost = const [];

class _ViewI24TwoWayFilhoHost0 extends import10.HostView<import1.I24TwoWayFilho> {
  @override
  void build() {
    this.componentView = ViewI24TwoWayFilho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I24TwoWayFilho();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I24TwoWayFilho> viewFactory_I24TwoWayFilhoHost0() {
  return _ViewI24TwoWayFilhoHost0();
}
