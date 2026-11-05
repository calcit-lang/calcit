
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'leaves $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn leaves (tree)
            hint-fn $ Fn Tree $ -> String
            match tree
              (String s) s
              (List _) |list
          :examples $ []
          :schema $ quote $ Fn Tree (-> String)
          :tests $ [] $ %{} 'TestEntry (:name |referred-union-name-is-the-declaration)
            :code $ quote $ do
              assert= |x $ leaves $ read-tree |x
              assert= |list $ leaves $ read-tree
                [] |a $ [] |b
            :tags $ #{} :referred-union :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (read-tree |x) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'read-tree $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-tree (x) (decode-map-as x Tree)
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Tree)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require $ app.model :refer $ Tree
    'app.model $ %{} 'FileEntry
      :defs $ {} $ 'Tree
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Tree String (List Tree)
          :examples $ []
          :schema $ quote UnionDef
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.model (:require)
