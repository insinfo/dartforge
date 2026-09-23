enum E<T> {
  v1<int>.named(),
  v2<int>.renamed();

  const E.named();
  const E.renamed() : this.named();
}
