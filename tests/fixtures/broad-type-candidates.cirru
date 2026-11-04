{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |broad-type-candidates
  :entries $ {} $ :default
    {} (:description "|W_BROAD_TYPE names a concrete type when every checked call passes one (#2566).") (:init-fn 'broad-type-candidates.main/main!) (:mode :native) (:reload-fn 'broad-type-candidates.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'broad-type-candidates.main
    %{} 'FileEntry
      :defs $ {}
        'Op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Op (:add Number) (:reset)
          :examples $ []
          :schema $ quote EnumDef
        'Msg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Msg (:text String)
          :examples $ []
          :schema $ quote EnumDef
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number)
          :examples $ []
          :schema $ quote StructDef
        'dispatch! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dispatch! (op) (println op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'mixed! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn mixed! (op) (println op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'unused! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unused! (op) (println op) &unit
          :examples $ []
          :schema $ quote $ Fn Enum (-> Unit)
        'show-point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-point (p) (println p) &unit
          :examples $ []
          :schema $ quote $ Fn Struct (-> Unit)
        'on-click $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-click (d!)
            d! $ %:: Op :add 1
            d! $ %:: Op :reset
            , &unit
          :examples $ []
          :schema $ quote $ Fn (Fn Enum (-> Unit)) (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            dispatch! $ %:: Op :add 1
            dispatch! $ %:: Op :reset
            mixed! $ %:: Op :reset
            mixed! $ %:: Msg :text |a
            show-point $ %{} Point (:x 1)
            on-click dispatch!
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns broad-type-candidates.main
