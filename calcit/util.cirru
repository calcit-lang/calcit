
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |util
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'util.core/main!) (:mode :native)
      :reload-fn 'util.core/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'util.core
    %{} 'FileEntry
      :defs $ {}
        'inside-eval: $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro inside-eval: (& body)
            if
              = :eval $ &get-calcit-running-mode
              quasiquote $ do (println "|env: eval") ~@body
              quasiquote $ do $ println "|env: not eval. tests skipped"
          :examples $ []
          :schema $ :: 'Macro $ {} (:rest 'Syntax)
            :capabilities $ #{} :platform-read
            :expansion $ :: 'Expr 'Dynamic
            :required $ []
        'inside-js: $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro inside-js: (& body)
            if
              not= :eval $ &get-calcit-running-mode
              quasiquote $ do (println "|env: js") ~@body
              quasiquote $ do $ println "|env: not js. tests skipped"
          :examples $ []
          :schema $ :: 'Macro $ {} (:rest 'Syntax)
            :capabilities $ #{} :platform-read
            :expansion $ :: 'Expr 'Dynamic
            :required $ []
        'log-title $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn log-title (title) (println) (println title) (println)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (:: 'Unit)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () (:: 'Unit)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns util.core (:require)
