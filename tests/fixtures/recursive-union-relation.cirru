{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :native) (:target :native)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :schema $ quote $ Fn (-> Unit)
        'MixedTree $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion MixedTree Number String (List MixedTree) (Map Tag MixedTree)
          :schema $ quote UnionDef
        'NumericTree $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion NumericTree Number (List NumericTree) (Map Tag NumericTree)
          :schema $ quote UnionDef
        'narrow $ %{} 'CodeEntry (:doc "|A MixedTree may hold strings, so it is not a NumericTree.")
          :code $ quote $ defn narrow (x) x
          :schema $ quote $ Fn (Map Tag MixedTree) (-> (Map Tag NumericTree))
        'EvenTree $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion EvenTree Number (List OddTree)
          :schema $ quote UnionDef
        'OddTree $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion OddTree Number (List EvenTree)
          :schema $ quote UnionDef
        'accept-mutual $ %{} 'CodeEntry (:doc "|Mutually recursive unions revisit two pairs in turn.")
          :code $ quote $ defn accept-mutual (x) x
          :schema $ quote $ Fn EvenTree (-> NumericTree)
        'accept $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn accept (x) x
          :schema $ quote $ Fn (Map Tag NumericTree) (-> (Map Tag MixedTree))
