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
          :code $ quote $ defn main! ()
            println (v-scale ([] 1 2 3) 2) (v-scale-method ([] 1 2 3) 2) (v-labels ([] 1 2 3)) (v-weighted ([] 1 2 3)) (v-alias-scale ([] 1 2 3) 2) (v-keep-big ([] 1 2 3)) (v-add ([] 1 2 3) ([] 1 1 1)) (v-alias-add ([] 1 2 3) ([] 1 1 1)) (list-add ([] 1 2) ([] 3 4 5))
            , &unit
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'Vec3 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Vec3 ([] 0 0 0)
          :schema $ quote $ Tuple Number Number Number
        'v-scale $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-scale (a k) (map a (fn (x) (&* x k)))
          :schema $ quote $ Fn (Tuple Number Number Number) Number (-> (Tuple Number Number Number))
        'v-scale-method $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-scale-method (a k) (.map a (fn (x) (&* x k)))
          :schema $ quote $ Fn (Tuple Number Number Number) Number (-> (Tuple Number Number Number))
        'v-labels $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-labels (a) (map a (fn (x) (str x)))
          :schema $ quote $ Fn (Tuple Number Number Number) (-> (Tuple String String String))
        'v-weighted $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-weighted (a) (map-indexed a (fn (i x) (&* i x)))
          :schema $ quote $ Fn (Tuple Number Number Number) (-> (Tuple Number Number Number))
        'v-alias-scale $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-alias-scale (a k) (map a (fn (x) (&* x k)))
          :schema $ quote $ Fn demo.main/Vec3 Number (-> demo.main/Vec3)
        'v-keep-big $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-keep-big (a) (filter a (fn (x) (&> x 1)))
          :schema $ quote $ Fn (Tuple Number Number Number) (-> (List Number))
        'v-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-add (a b) (zip-with a b &+)
          :schema $ quote $ Fn (Tuple Number Number Number) (Tuple Number Number Number) (-> (Tuple Number Number Number))
        'v-alias-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-alias-add (a b) (zip-with a b (fn (x y) (&+ x y)))
          :schema $ quote $ Fn demo.main/Vec3 demo.main/Vec3 (-> demo.main/Vec3)
        'list-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn list-add (a b) (zip-with a b &+)
          :schema $ quote $ Fn (List Number) (List Number) (-> (List Number))
