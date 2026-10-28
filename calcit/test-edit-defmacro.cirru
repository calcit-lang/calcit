
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
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (+ 1 2)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
          :tests $ []
            %{} 'TestEntry (:name |required-expands)
              :code $ quote $ assert= 1 (required-id 1)
              :tags $ #{} :macro-edit
            %{} 'TestEntry (:name |optional-expands)
              :code $ quote $ assert= 2 (optional-id 2 20)
              :tags $ #{} :macro-edit-compat
            %{} 'TestEntry (:name |rest-expands)
              :code $ quote $ assert= 3 (rest-id 3 4 5)
              :tags $ #{} :macro-edit
        'optional-id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro optional-id (value ? ignored) value
          :examples $ []
          :schema $ quote $ Macro Syntax
            -> $ Expr Dynamic
            :optional Syntax
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ quote $ Fn (-> Nil)
        'required-id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro required-id (value) value
          :examples $ []
          :schema $ quote $ Macro Syntax
            -> $ Expr Dynamic
        'rest-id $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro rest-id (value & ignored) value
          :examples $ []
          :schema $ quote $ Macro Syntax & Syntax
            -> $ Expr Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
