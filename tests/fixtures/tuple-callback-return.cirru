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
          :code $ quote $ defstruct Task (:title String) (:done Bool)
          :schema $ quote StructDef
        'direct $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn direct (t) ([] (:title t) (:done t))
          :schema $ quote $ Fn Task (-> (Tuple String Bool))
        'lam $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lam () $ fn (t) ([] (:title t) (:done t))
          :schema $ quote $ Fn (-> (Fn Task (-> (Tuple String Bool))))
        'q05 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q05 (ts)
            .map ts $ fn (t) ([] (:title t) (:done t))
          :schema $ quote $ Fn (List Task) (-> (List (Tuple String Bool)))
        'q-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-map (ts)
            map ts $ fn (t) ([] (:title t) (:done t))
          :schema $ quote $ Fn (List Task) (-> (List (Tuple String Bool)))
        'q-let $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-let (ts)
            .map ts $ fn (t)
              let
                  title $ :title t
                [] title (:done t)
          :schema $ quote $ Fn (List Task) (-> (List (Tuple String Bool)))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
