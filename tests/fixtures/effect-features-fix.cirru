
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |effect-features-fix
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'effect-features-fix.main/main!) (:mode :native) (:reload-fn 'effect-features-fix.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'effect-features-fix.main
    %{} 'FileEntry
      :defs $ {}
        '*store $ %{} 'CodeEntry (:doc "|Global state.")
          :code $ quote $ defref *store 0
          :examples $ []
          :schema $ quote $ Ref Number
        'both $ %{} 'CodeEntry (:doc "|Needs :state and :io through its callees.")
          :code $ quote $ defn both ()
            step-b $ read-store
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'log-line $ %{} 'CodeEntry (:doc "|Already declares :io.")
          :code $ quote $ defn log-line (x) (println x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit) (:features :io)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (both) (pure 1) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'pure $ %{} 'CodeEntry (:doc "|No effect, no change.")
          :code $ quote $ defn pure (x) (+ x 1)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'read-store $ %{} 'CodeEntry (:doc "|Reads a global Ref.")
          :code $ quote $ defn read-store () (deref *store)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'step-a $ %{} 'CodeEntry (:doc "|Inherits :io from log-line.")
          :code $ quote $ defn step-a (x) (log-line x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
        'step-b $ %{} 'CodeEntry (:doc "|Inherits :io two calls away.")
          :code $ quote $ defn step-b (x) (step-a x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns effect-features-fix.main
