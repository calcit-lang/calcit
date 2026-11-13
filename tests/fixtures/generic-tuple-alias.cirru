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
            println (pair 1 |a) (swap 1 |a) (v-add ([] 1 2 3) ([] 1 1 1)) (v-len2 ([] 3 4 0)) (v-count ([] 1 2 3))
            , &unit
          :schema $ quote $ Fn (-> Unit) (:features :io)
        'Vec3 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Vec3 ([] 0 0 0)
          :schema $ quote $ Tuple Number Number Number
        'pair $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn pair (a b) ([] a b)
          :schema $ quote $ Fn A B (-> (Tuple A B)) (:generics A B)
        'swap $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn swap (a b) ([] b a)
          :schema $ quote $ Fn A B (-> (Tuple B A)) (:generics A B)
        'v-add $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-add (a b)
            let[] (x y z) a
              let[] (p q r) b
                [] (&+ x p) (&+ y q) (&+ z r)
          :schema $ quote $ Fn demo.main/Vec3 demo.main/Vec3 (-> demo.main/Vec3)
        'v-len2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-len2 (a) (&+ (&* (nth a 0) (nth a 0)) (&* (nth a 1) (nth a 1)))
          :schema $ quote $ Fn demo.main/Vec3 (-> Number)
        'v-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn v-count (a) (count a)
          :schema $ quote $ Fn demo.main/Vec3 (-> Number)
