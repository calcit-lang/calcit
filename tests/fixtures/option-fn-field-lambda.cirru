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
        'Click $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Click (:x Number)
          :schema $ quote StructDef
        'Props $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Props (:label String) (:on-click (Option (Fn Click (-> Unit))))
          :schema $ quote StructDef
        'fire $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn fire (props)
            match (:on-click props)
              (:some f) (f (Click :x 1))
              (:none) (println |none)
            , &unit
          :schema $ quote $ Fn Props (-> Unit)
        'map-literal $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-literal ()
            fire $ {} (:label |a)
              :on-click $ fn (e) (println |map (:x e))
          :schema $ quote $ Fn (-> Unit)
        'map-literal-some $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn map-literal-some ()
            fire $ {} (:label |b)
              :on-click $ Some $ fn (e) (println |map-some (:x e))
          :schema $ quote $ Fn (-> Unit)
        'direct-ctor $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn direct-ctor ()
            Props :label |c :on-click $ fn (e) (println |ctor (:x e))
          :schema $ quote $ Fn (-> Props)
        'direct-ctor-some $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn direct-ctor-some ()
            Props :label |d :on-click $ Some $ fn (e) (println |ctor-some (:x e))
          :schema $ quote $ Fn (-> Props)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            map-literal
            map-literal-some
            fire $ direct-ctor
            fire $ direct-ctor-some
            fire $ Props :label |e
          :schema $ quote $ Fn (-> Unit)
