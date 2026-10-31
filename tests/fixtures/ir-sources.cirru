{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.empty $ %{} 'FileEntry
      :defs $ {}
      :ns $ %{} 'NsEntry (:doc "|Namespace without definitions")
        :code $ quote $ ns app.empty
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x 'Number)
          :examples $ []
          :schema $ quote StructDef
        'double $ %{} 'CodeEntry (:doc "|Twice the input")
          :code $ quote $ defn double (n)
            let
                twice $ * n 2
              , twice
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (double 21)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'read-point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-point (raw) (decode-map-as raw 'app.main/Point)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> app.main/Point)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'unused $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unused (s) (str s |!)
          :examples $ []
          :schema $ quote $ Fn String (-> String)
      :ns $ %{} 'NsEntry (:doc "|IR sources fixture")
        :code $ quote $ ns app.main (:require)
