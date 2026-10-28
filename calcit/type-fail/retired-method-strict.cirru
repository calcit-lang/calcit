
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-retired-method-strict
  :entries $ {}
    :contains-list $ {} (:description |)
      :init-fn 'type-fail-retired-method-strict.main/run-contains-list
      :mode :native
      :reload-fn 'type-fail-retired-method-strict.main/reload!
      :target :node
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :contains-string $ {} (:description |)
      :init-fn 'type-fail-retired-method-strict.main/run-contains-string
      :mode :native
      :reload-fn 'type-fail-retired-method-strict.main/reload!
      :target :node
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :default $ {} (:description |)
      :init-fn 'type-fail-retired-method-strict.main/run-join-list
      :mode :native
      :reload-fn 'type-fail-retired-method-strict.main/reload!
      :target :node
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :join-list $ {} (:description |)
      :init-fn 'type-fail-retired-method-strict.main/run-join-list
      :mode :native
      :reload-fn 'type-fail-retired-method-strict.main/reload!
      :target :node
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :values-map $ {} (:description |)
      :init-fn 'type-fail-retired-method-strict.main/run-values-map
      :mode :native
      :reload-fn 'type-fail-retired-method-strict.main/reload!
      :target :node
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-retired-method-strict.main
    %{} 'FileEntry
      :defs $ {}
        'contains-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn contains-list (xs) (xs .contains? 1)
          :examples $ []
          :schema $ quote $ Fn (List Number) (-> Dynamic)
        'contains-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn contains-string (s) (s .contains? 1)
          :examples $ []
          :schema $ quote $ Fn String (-> Dynamic)
        'join-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn join-list (xs) (xs .join 0)
          :examples $ []
          :schema $ quote $ Fn (List Number) (-> Dynamic)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'run-contains-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-contains-list ()
            contains-list $ [] 1 2
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'run-contains-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-contains-string () (contains-string |abc)
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'run-join-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-join-list ()
            join-list $ [] 1 2
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'run-values-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn run-values-map ()
            values-map $ &{} :a 1
          :examples $ []
          :schema $ quote $ Fn (-> Dynamic)
        'values-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn values-map (m) (m .values)
          :examples $ []
          :schema $ quote $ Fn (Map Tag Number) (-> Dynamic)
      :ns $ %{} 'NsEntry (:doc "|Strict retired method fixture.")
        :code $ quote $ ns type-fail-retired-method-strict.main
