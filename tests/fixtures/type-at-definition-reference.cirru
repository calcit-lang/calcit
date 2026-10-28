
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-at-reference
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'comp-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defcomp comp-styled (a) (merge ui/global column ui/noisy)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'defcomp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro defcomp (comp-name params & body)
            quasiquote $ defn (~ comp-name) (~ params)
              wrap-it $ let
                  result $ do $ ~@ body
                , result
          :examples $ []
          :schema $ quote $ Macro Syntax Syntax & Syntax
            -> $ Expr Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (comp-styled 1) (plain-styled 1) (shadowed 1) (opaque-styled) (quoted)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'opaque-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn opaque-styled ()
            &list:first $ [] ui/opaque
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'plain-styled $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plain-styled (a) (merge ui/global column)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'quoted $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn quoted () (quote ui/global)
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'shadowed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defcomp shadowed (column)
            &let (global 1) (str column global)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'wrap-it $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn wrap-it (x) x
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Dynamic)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require (app.ui :as ui)
            app.ui :refer $ column global
    'app.ui $ %{} 'FileEntry
      :defs $ {}
        'column $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def column
            {} $ :b 2
          :examples $ []
          :schema $ quote $ Map Tag Number
        'global $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def global
            {} $ :a 1
          :examples $ []
          :schema $ quote $ Map Tag Number
        'noisy $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def noisy
            do (println "|SIDE EFFECT FROM DEFINITION")
              {} $ :c 3
          :examples $ []
          :schema $ quote $ Map Tag Number
        'opaque $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def opaque (parse-cirru-edn "|{} (:d 4)")
          :examples $ []
          :schema $ quote Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.ui
