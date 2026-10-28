
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |test-hygienic
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'test-hygienic.main/main!) (:mode :native) (:reload-fn 'test-hygienic.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-hygienic.lib $ %{} 'FileEntry
      :defs $ {}
        'add-11 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro add-11 (a b)
            let
                c 11
              println "|internal c:" a b c
              quasiquote $ do (println "|c is:" c)
                [] (~ a) (~ b) c (~ c) (add-2 8)
          :examples $ []
          :schema $ quote $ Macro (Expr Number) (Expr Number)
            -> $ Expr $ List Number
            :capabilities :log
        'add-11-safe $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro add-11-safe (a b)
            with-gensyms (c)
              quasiquote $ do $ &let (~c 11)
                [] (~ a) (~ b) ~c
          :examples $ []
          :schema $ quote $ Macro (Expr Number) (Expr Number)
            -> $ Expr $ List Number
        'add-2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn add-2 (x) (&+ x 2)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-hygienic.lib
    'test-hygienic.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (try-hygienic)
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
        'try-hygienic $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn try-hygienic () (println "|Testing hygienic")
            let
                c 4
              assert= (add-11 1 2) ([] 1 2 4 11 10)
              assert= (add-11-safe 1 2) ([] 1 2 11)
              , true
          :examples $ []
          :schema $ quote $ Fn (-> Bool)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns test-hygienic.main
          :require $ test-hygienic.lib :refer $ add-11 add-11-safe
