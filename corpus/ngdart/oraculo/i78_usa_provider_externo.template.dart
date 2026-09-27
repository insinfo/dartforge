// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i78_usa_provider_externo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i78_usa_provider_externo.dart' as import1;
import 'package:ngdart/src/utilities.dart' as import2;
import 'package:ngdart/src/di/errors.dart' as import3;
import 'i67_provider_externo.dart' as import4;
import 'package:ngdart/src/core/zone/ng_zone.dart' as import5;
import 'i67_provider_externo.template.dart' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I78UsaProviderExterno = const [];

class ViewI78UsaProviderExterno0 extends import0.ComponentView<import1.I78UsaProviderExterno> {
  late dynamic _I67Local_0_6 = (import2.isDevMode
      ? import3.debugInjectorWrap(import4.I67Local, () {
          return import4.I67Local((this.parentView!).injectorGet(import5.NgZone, this.parentIndex));
        })
      : import4.I67Local((this.parentView!).injectorGet(import5.NgZone, this.parentIndex)));
  late dynamic _I67Apelido_0_7 = (this.parentView!).injectorGet(import4.I67Externo, this.parentIndex);
  late final import6.ViewI67ProviderExterno0 _compView_0;
  late final import4.I67ProviderExterno _I67ProviderExterno_0_5;
  static import7.ComponentStyles? _componentStyles;
  ViewI78UsaProviderExterno0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import2.unsafeCast(import10.document.createElement('i78-usa-provider-externo'));
  }
  static String? get _debugComponentUrl {
    return (import2.isDevMode ? 'asset:corpus_ngdart/lib/src/i78_usa_provider_externo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import6.ViewI67ProviderExterno0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I67ProviderExterno_0_5 = import4.I67ProviderExterno();
    this._compView_0.create(this._I67ProviderExterno_0_5);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import4.I67Local)) {
        return this._I67Local_0_6;
      }
      if (identical(token, import4.I67Apelido)) {
        return this._I67Apelido_0_7;
      }
    }
    return notFoundResult;
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
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I78UsaProviderExterno, _debugComponentUrl));
      if (import2.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I78UsaProviderExternoNgFactory = ComponentFactory<import1.I78UsaProviderExterno>('i78-usa-provider-externo', viewFactory_I78UsaProviderExternoHost0);
ComponentFactory<import1.I78UsaProviderExterno> get I78UsaProviderExternoNgFactory {
  return _I78UsaProviderExternoNgFactory;
}

ComponentFactory<import1.I78UsaProviderExterno> createI78UsaProviderExternoFactory() {
  return ComponentFactory('i78-usa-provider-externo', viewFactory_I78UsaProviderExternoHost0);
}

final List<Object> styles$I78UsaProviderExternoHost = const [];

class _ViewI78UsaProviderExternoHost0 extends import12.HostView<import1.I78UsaProviderExterno> {
  @override
  void build() {
    this.componentView = ViewI78UsaProviderExterno0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I78UsaProviderExterno();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I78UsaProviderExterno> viewFactory_I78UsaProviderExternoHost0() {
  return _ViewI78UsaProviderExternoHost0();
}
