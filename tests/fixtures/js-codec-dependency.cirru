
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.leaf $ %{} 'FileEntry
      :defs $ {} $ 'Count
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Count (:value Number)
          :examples $ []
          :schema $ quote StructDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.leaf
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'decode-thing $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn decode-thing (data) (decode-map-as data app.model/Thing)
          :examples $ []
          :schema $ quote $ Fn (Map Tag String) (-> app.model/Thing)
          :tests $ [] $ %{} 'TestEntry (:name |decode-nominal)
            :code $ quote $ assert= (app.model/Thing :name |Ada)
              decode-thing $ {} $ :name |Ada
            :tags $ #{} :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (+ 1 2)
          :examples $ []
          :schema $ quote $ Fn (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |independent-entry)
            :code $ quote $ assert= 3 (main!)
            :tags $ #{} :unit
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
    'app.model $ %{} 'FileEntry
      :defs $ {}
        'Thing $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Thing (:name String)
          :examples $ []
          :schema $ quote StructDef
        'parse-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn parse-count (source) (parse-cirru-edn-as source app.leaf/Count)
          :examples $ []
          :schema $ quote $ Fn String (-> app.leaf/Count)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.model
    'app.unrelated $ %{} 'FileEntry
      :defs $ {} $ 'value
        %{} 'CodeEntry (:doc |)
          :code $ quote $ def value 42
          :examples $ []
          :schema $ quote Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.unrelated
