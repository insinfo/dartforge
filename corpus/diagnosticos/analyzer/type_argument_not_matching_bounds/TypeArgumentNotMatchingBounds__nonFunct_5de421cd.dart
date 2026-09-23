class A {}
class B extends A {}
class G<T extends A> {}
typedef X = G<B>;
