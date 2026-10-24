{}
  :about "|Fixture for query type-at on compiler-resolved definition references."
  :package |type-at-reference
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.ui $ %{} 'FileEntry
      :defs $ {}
        'column $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def column $ {} (:b 2)
          :examples $ []
          :schema $ :: 'Map 'Tag 'Number
        'global $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def global $ {} (:a 1)
          :examples $ []
          :schema $ :: 'Map 'Tag 'Number
        'noisy $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def noisy $ do (println "|SIDE EFFECT FROM DEFINITION") ({} (:c 3))
          :examples $ []
          :schema $ :: 'Map 'Tag 'Number
        'opaque $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def opaque $ parse-cirru-edn "|{} (:d 4)"
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.ui
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'defcomp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro defcomp (comp-name params & body)
            quasiquote $ defn (~ comp-name) (~ params)
              wrap-it $ let
                  result $ do $ ~@ body
                , result
          :examples $ []
          :schema $ :: 'Macro $ {} (:rest 'Syntax)
            :capabilities $ #{}
            :expansion $ :: 'Expr 'Dynamic
            :required $ [] 'Syntax 'Syntax
        'comp-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defcomp comp-styled (a)
            merge ui/global column ui/noisy
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'Dynamic)
        'plain-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plain-styled (a)
            merge ui/global column
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'Dynamic)
        'shadowed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defcomp shadowed (column)
            &let (global 1) (str column global)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'Dynamic)
        'opaque-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn opaque-styled () (&list:first $ [] ui/opaque)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [])
        'quoted $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn quoted () $ quote ui/global
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [])
        'wrap-it $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-it (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic) (:args $ [] 'Dynamic)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (comp-styled 1) (plain-styled 1) (shadowed 1) (opaque-styled) (quoted)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit) (:args $ [])
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main $ :require (app.ui :as ui) (app.ui :refer $ column global)
