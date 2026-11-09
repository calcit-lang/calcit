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
        'q10 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q10 (ts)
            .foldl ts (#{}) $ fn (acc t) (include acc $ :title t)
          :schema $ quote $ Fn (List Task) (-> (Set String))
        'q-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-list (ts)
            .foldl ts ([]) $ fn (acc t) (append acc $ :title t)
          :schema $ quote $ Fn (List Task) (-> (List String))
        'q-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-map (ts)
            .foldl ts ({}) $ fn (acc t) (assoc acc (:title t) (:done t))
          :schema $ quote $ Fn (List Task) (-> (Map String Bool))
        'q-prefix $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-prefix (ts)
            foldl ts (#{}) $ fn (acc t) (include acc $ :title t)
          :schema $ quote $ Fn (List Task) (-> (Set String))
        'q-let $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn q-let (ts)
            let
                out $ .foldl ts (#{}) $ fn (acc t) (include acc $ :title t)
              , out
          :schema $ quote $ Fn (List Task) (-> (Set String))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
