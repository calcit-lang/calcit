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
        'apply-task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-task (t f) (f t)
          :schema $ quote $ Fn Task (Fn Task (-> String)) (-> String)
        'h1-user-fn $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h1-user-fn (t)
            apply-task t $ fn (x)
              hint-fn $ Fn Task (-> String)
              :title x
          :schema $ quote $ Fn Task (-> String)
        'h2-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h2-filter (ts)
            .filter ts $ fn (x)
              hint-fn $ Fn Task (-> Bool)
              :done x
          :schema $ quote $ Fn (List Task) (-> (List Task))
        'h3-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h3-map (ts)
            .map ts $ fn (x)
              hint-fn $ Fn Task (-> String)
              :title x
          :schema $ quote $ Fn (List Task) (-> (List String))
        'h4-narrower $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h4-narrower (xs)
            .map xs $ fn (x)
              hint-fn $ Fn Dynamic (-> Number)
              , 1
          :schema $ quote $ Fn (List Number) (-> (List Number))
        'h5-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h5-tail ()
            fn (x)
              hint-fn $ Fn Task (-> String)
              :title x
          :schema $ quote $ Fn (-> (Fn Task (-> String)))
        'h6-bound-once $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h6-bound-once (ts)
            let
                label $ fn (x)
                  hint-fn $ Fn Task (-> String)
                  :title x
              .map ts label
          :schema $ quote $ Fn (List Task) (-> (List String))
        'h7-each $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h7-each (ts)
            each ts $ fn (x)
              hint-fn $ Fn Task (-> Unit)
              println $ :title x
          :schema $ quote $ Fn (List Task) (-> Unit) (:features :io)
        'Patch $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Patch (:mount (Fn Task (-> String))) (:noop)
          :schema $ quote EnumDef
        'h8-enum-payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn h8-enum-payload ()
            %:: Patch :mount $ fn (x)
              hint-fn $ Fn Task (-> String)
              :title x
          :schema $ quote $ Fn (-> Patch)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                t $ Task :title |a :done true
              println (h1-user-fn t) (h2-filter $ [] t) (h3-map $ [] t) (h4-narrower $ [] 1) ((h5-tail) t) (h6-bound-once $ [] t)
              h7-each $ [] t
              h8-enum-payload
              , &unit
          :schema $ quote $ Fn (-> Unit) (:features :io)
