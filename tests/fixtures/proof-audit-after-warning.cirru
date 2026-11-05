
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.proof/main!) (:mode :native) (:reload-fn 'app.proof/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.proof
    %{} 'FileEntry
      :defs $ {}
        'Scalar $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Scalar Number String
          :examples $ []
          :schema $ quote UnionDef
        'bad-call $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-call () (take-string 1) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'bad-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bad-map ()
            {} (:n 1) (:bad true)
          :examples $ []
          :schema $ quote $ Fn
            -> $ Map Tag app.proof/Scalar
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'take-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn take-string (value) &unit
          :examples $ []
          :schema $ quote $ Fn String (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.proof (:require)
