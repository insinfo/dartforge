// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j61_provedor_do_proprio_no.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j61_provedor_do_proprio_no.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/di/errors.dart' as import11;

final List<Object> styles$J61ComConfig = const [];

class ViewJ61ComConfig0 extends import0.ComponentView<import1.J61ComConfig> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ61ComConfig0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j61-com-config'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j61_provedor_do_proprio_no.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateTextWithPrimitive(_ctx.config.n) /* REF:asset:corpus_ngdart/lib/src/j61_provedor_do_proprio_no.dart:378:390 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J61ComConfig, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J61ComConfigNgFactory = ComponentFactory<import1.J61ComConfig>('j61-com-config', viewFactory_J61ComConfigHost0);
ComponentFactory<import1.J61ComConfig> get J61ComConfigNgFactory {
  return _J61ComConfigNgFactory;
}

ComponentFactory<import1.J61ComConfig> createJ61ComConfigFactory() {
  return ComponentFactory('j61-com-config', viewFactory_J61ComConfigHost0);
}

final List<Object> styles$J61ComConfigHost = const [];

class _ViewJ61ComConfigHost0 extends import10.HostView<import1.J61ComConfig> {
  late final import1.J61Config _J61Config_0_5;
  @override
  void build() {
    this.componentView = ViewJ61ComConfig0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._J61Config_0_5 = import1.J61Config();
    this.component = (import6.isDevMode
        ? import11.debugInjectorWrap(import1.J61ComConfig, () {
            return import1.J61ComConfig(this.injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_0_5);
          })
        : import1.J61ComConfig(this.injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_0_5));
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J61Config) && (0 == nodeIndex))) {
      return this._J61Config_0_5;
    }
    return notFoundResult;
  }
}

import10.HostView<import1.J61ComConfig> viewFactory_J61ComConfigHost0() {
  return _ViewJ61ComConfigHost0();
}

final List<Object> styles$J61ComDois = const [];

class ViewJ61ComDois0 extends import0.ComponentView<import1.J61ComDois> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ61ComDois0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j61-com-dois'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j61_provedor_do_proprio_no.dart' : null);
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
    this._textBinding_1.updateTextWithPrimitive(_ctx.config.n) /* REF:asset:corpus_ngdart/lib/src/j61_provedor_do_proprio_no.dart:736:748 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J61ComDois, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J61ComDoisNgFactory = ComponentFactory<import1.J61ComDois>('j61-com-dois', viewFactory_J61ComDoisHost0);
ComponentFactory<import1.J61ComDois> get J61ComDoisNgFactory {
  return _J61ComDoisNgFactory;
}

ComponentFactory<import1.J61ComDois> createJ61ComDoisFactory() {
  return ComponentFactory('j61-com-dois', viewFactory_J61ComDoisHost0);
}

final List<Object> styles$J61ComDoisHost = const [];

class _ViewJ61ComDoisHost0 extends import10.HostView<import1.J61ComDois> {
  late final import1.J61Servico _J61Servico_0_5;
  late final import1.J61Config _J61Config_0_6;
  @override
  void build() {
    this.componentView = ViewJ61ComDois0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._J61Servico_0_5 = import1.J61Servico();
    this._J61Config_0_6 = import1.J61Config();
    this.component = import1.J61ComDois(this._J61Servico_0_5, this._J61Config_0_6);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J61Servico)) {
        return this._J61Servico_0_5;
      }
      if (identical(token, import1.J61Config)) {
        return this._J61Config_0_6;
      }
    }
    return notFoundResult;
  }
}

import10.HostView<import1.J61ComDois> viewFactory_J61ComDoisHost0() {
  return _ViewJ61ComDoisHost0();
}

final List<Object> styles$J61ProvedorDoProprioNo = const [];

class ViewJ61ProvedorDoProprioNo0 extends import0.ComponentView<import1.J61ProvedorDoProprioNo> {
  late final ViewJ61ComConfig0 _compView_1;
  late final import1.J61Config _J61Config_1_5;
  late final import1.J61ComConfig _J61ComConfig_1_6;
  late final ViewJ61ComDois0 _compView_2;
  late final import1.J61Servico _J61Servico_2_5;
  late final import1.J61Config _J61Config_2_6;
  late final import1.J61ComDois _J61ComDois_2_7;
  late final ViewJ61ComConfig0 _compView_3;
  late final import1.J61Config _J61Config_3_5;
  late final import1.J61ComConfig _J61ComConfig_3_6;
  static import3.ComponentStyles? _componentStyles;
  ViewJ61ProvedorDoProprioNo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j61-provedor-do-proprio-no'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j61_provedor_do_proprio_no.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this._compView_1 = ViewJ61ComConfig0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._J61Config_1_5 = import1.J61Config();
    this._J61ComConfig_1_6 = (import6.isDevMode
        ? import11.debugInjectorWrap(import1.J61ComConfig, () {
            return import1.J61ComConfig((this.parentView!).injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_1_5);
          })
        : import1.J61ComConfig((this.parentView!).injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_1_5));
    this._compView_1.create(this._J61ComConfig_1_6);
    this._compView_2 = ViewJ61ComDois0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._J61Servico_2_5 = import1.J61Servico();
    this._J61Config_2_6 = import1.J61Config();
    this._J61ComDois_2_7 = import1.J61ComDois(this._J61Servico_2_5, this._J61Config_2_6);
    this._compView_2.create(this._J61ComDois_2_7);
    this._compView_3 = ViewJ61ComConfig0(this, 3);
    final _el_3 = this._compView_3.rootElement;
    parentRenderNode.append(_el_3);
    this._J61Config_3_5 = import1.J61Config();
    this._J61ComConfig_3_6 = (import6.isDevMode
        ? import11.debugInjectorWrap(import1.J61ComConfig, () {
            return import1.J61ComConfig((this.parentView!).injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_3_5);
          })
        : import1.J61ComConfig((this.parentView!).injectorGet(import1.J61Externo, this.parentIndex), this._J61Config_3_5));
    this._compView_3.create(this._J61ComConfig_3_6);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J61Config) && (1 == nodeIndex))) {
      return this._J61Config_1_5;
    }
    if ((2 == nodeIndex)) {
      if (identical(token, import1.J61Servico)) {
        return this._J61Servico_2_5;
      }
      if (identical(token, import1.J61Config)) {
        return this._J61Config_2_6;
      }
    }
    if ((identical(token, import1.J61Config) && (3 == nodeIndex))) {
      return this._J61Config_3_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
    this._compView_3.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
    this._compView_3.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J61ProvedorDoProprioNo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J61ProvedorDoProprioNoNgFactory = ComponentFactory<import1.J61ProvedorDoProprioNo>('j61-provedor-do-proprio-no', viewFactory_J61ProvedorDoProprioNoHost0);
ComponentFactory<import1.J61ProvedorDoProprioNo> get J61ProvedorDoProprioNoNgFactory {
  return _J61ProvedorDoProprioNoNgFactory;
}

ComponentFactory<import1.J61ProvedorDoProprioNo> createJ61ProvedorDoProprioNoFactory() {
  return ComponentFactory('j61-provedor-do-proprio-no', viewFactory_J61ProvedorDoProprioNoHost0);
}

final List<Object> styles$J61ProvedorDoProprioNoHost = const [];

class _ViewJ61ProvedorDoProprioNoHost0 extends import10.HostView<import1.J61ProvedorDoProprioNo> {
  late import1.J61Externo _J61Externo_0_6 = import1.J61Externo();
  @override
  void build() {
    this.componentView = ViewJ61ProvedorDoProprioNo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J61ProvedorDoProprioNo();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J61Externo) && (0 == nodeIndex))) {
      return this._J61Externo_0_6;
    }
    return notFoundResult;
  }
}

import10.HostView<import1.J61ProvedorDoProprioNo> viewFactory_J61ProvedorDoProprioNoHost0() {
  return _ViewJ61ProvedorDoProprioNoHost0();
}
