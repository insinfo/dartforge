class A<X> {}
class B<X> extends A<void Function<Y extends X>()> {}
//      ^
// [diag.wrongTypeParameterVarianceInSuperinterface] 'X' can't be used contravariantly or invariantly in 'A<void Function<Y extends X>()>'.
