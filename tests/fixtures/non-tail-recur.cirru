
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'bad-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-alias ()
            loop
                a 1
              let
                  r recur
                if (> a 3) a $ r $ inc a
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-before-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-before-tail ()
            loop
                a 1
              if (> a 3) a $ do
                recur $ inc a
                , 100
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-from-macro $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-from-macro ()
            loop
                a 1
              if (> a 3) a $ list-recur $ inc a
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-in-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-in-list ()
            loop
                a 1
              if (> a 3) a $ [] $ recur (inc a)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-in-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-in-match ()
            loop
                a 1
              match (:: :some a)
                (:some x)
                  if (> x 3) x $ [] $ recur (inc x)
                _ 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-in-str $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-in-str ()
            loop
                a 1
              if (> a 3) a $ str $ recur (inc a)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'bad-in-try $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-in-try ()
            loop
                a 1
              try
                if (> a 3) a $ [] $ recur (inc a)
                fn (e) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'list-recur $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro list-recur (x)
            quasiquote $ [] $ recur ~x
          :examples $ []
          :schema $ :: 'Macro $ {}
            :capabilities $ #{}
            :expansion $ :: 'Expr 'Dynamic
            :required $ [] 'Syntax
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ ok-loop
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'ok-in-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn ok-in-match ()
            loop
                a 1
              match (:: :some a)
                (:some x)
                  if (> x 3) x $ recur $ inc x
                _ 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'ok-in-try $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn ok-in-try ()
            loop
                a 1
              try
                if (> a 3) a $ recur $ inc a
                fn (e) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'ok-loop $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn ok-loop ()
            loop
                a 1
              if (> a 3) a $ recur $ inc a
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (println |reload)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
