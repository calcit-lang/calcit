{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :js) (:target :browser)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'Task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Task (:title String)
          :schema $ quote StructDef
        'Patch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Patch (:mount (Fn Task (-> String))) (:noop)
          :schema $ quote EnumDef
        'anonymous-ctor $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn anonymous-ctor () $ %:: Patch :mount $ fn (t) (:titel t)
          :schema $ quote $ Fn (-> Patch)
        'named-ctor $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn named-ctor () $ Patch :mount $ fn (t) (:titel t)
          :schema $ quote $ Fn (-> Patch)
        'Mapper $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Mapper ([] T) (:pick (Fn Task (-> T)))
          :schema $ quote EnumDef
        'generic-ctor $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn generic-ctor () $ Mapper :pick $ fn (t) (:titel t)
          :schema $ quote $ Fn (-> (Mapper String))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (anonymous-ctor) (named-ctor) (generic-ctor) nil
          :schema $ quote $ Fn (-> Nil)
