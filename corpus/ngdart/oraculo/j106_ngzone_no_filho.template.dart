// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j106_ngzone_no_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j106_ngzone_no_filho.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/core/zone/ng_zone.dart' as import11;
import 'package:ngdart/src/core/application_ref.dart' as import12;

final List<Object> styles$J106Faixa = const [];

class ViewJ106Faixa0 extends import0.ComponentView<import1.J106Faixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ106Faixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j106-faixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j106_ngzone_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J106Faixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J106FaixaNgFactory = ComponentFactory<import1.J106Faixa>('j106-faixa', viewFactory_J106FaixaHost0);
ComponentFactory<import1.J106Faixa> get J106FaixaNgFactory {
  return _J106FaixaNgFactory;
}

ComponentFactory<import1.J106Faixa> createJ106FaixaFactory() {
  return ComponentFactory('j106-faixa', viewFactory_J106FaixaHost0);
}

final List<Object> styles$J106FaixaHost = const [];

class _ViewJ106FaixaHost0 extends import9.HostView<import1.J106Faixa> {
  @override
  void build() {
    this.componentView = ViewJ106Faixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J106Faixa, () {
            return import1.J106Faixa(this.componentView, this.injectorGet(import11.NgZone, this.parentIndex), this.injectorGetOptional(import12.ApplicationRef, this.parentIndex));
          })
        : import1.J106Faixa(this.componentView, this.injectorGet(import11.NgZone, this.parentIndex), this.injectorGetOptional(import12.ApplicationRef, this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J106Faixa> viewFactory_J106FaixaHost0() {
  return _ViewJ106FaixaHost0();
}

final List<Object> styles$J106Usa = const [];

class ViewJ106Usa0 extends import0.ComponentView<import1.J106Usa> {
  late final ViewJ106Faixa0 _compView_0;
  late final import1.J106Faixa _J106Faixa_0_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ106Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j106-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j106_ngzone_no_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ106Faixa0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J106Faixa_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J106Faixa, () {
            return import1.J106Faixa(this._compView_0, (this.parentView!).injectorGet(import11.NgZone, this.parentIndex), (this.parentView!).injectorGetOptional(import12.ApplicationRef, this.parentIndex));
          })
        : import1.J106Faixa(this._compView_0, (this.parentView!).injectorGet(import11.NgZone, this.parentIndex), (this.parentView!).injectorGetOptional(import12.ApplicationRef, this.parentIndex)));
    this._compView_0.create(this._J106Faixa_0_5);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J106Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J106UsaNgFactory = ComponentFactory<import1.J106Usa>('j106-usa', viewFactory_J106UsaHost0);
ComponentFactory<import1.J106Usa> get J106UsaNgFactory {
  return _J106UsaNgFactory;
}

ComponentFactory<import1.J106Usa> createJ106UsaFactory() {
  return ComponentFactory('j106-usa', viewFactory_J106UsaHost0);
}

final List<Object> styles$J106UsaHost = const [];

class _ViewJ106UsaHost0 extends import9.HostView<import1.J106Usa> {
  @override
  void build() {
    this.componentView = ViewJ106Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J106Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J106Usa> viewFactory_J106UsaHost0() {
  return _ViewJ106UsaHost0();
}
