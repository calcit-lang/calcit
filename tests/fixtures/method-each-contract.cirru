{}
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'Res $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Res (:kind 'Tag) (:id 'String)
          :examples $ []
          :schema $ quote StructDef
        'check-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-all (previous next)
            hint-fn $ Fn (List app.main/Res) (List app.main/Res) (-> Unit)
            .each (concat previous next)
              fn (r)
                hint-fn $ Fn app.main/Res (-> Unit)
                assert |kind $ = (:kind r) :font
          :examples $ []
          :schema $ quote $ Fn (List app.main/Res) (List app.main/Res) (-> Unit)
        'check-tags $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn check-tags ()
            hint-fn $ Fn (-> Unit)
            .each ([] :code :delay)
              fn (t)
                hint-fn $ Fn Tag (-> Unit)
                assert |tag $ tag? t
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            hint-fn $ Fn (-> Unit)
            check-all ([] (Res :kind :font :id |a)) ([])
            check-tags
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
