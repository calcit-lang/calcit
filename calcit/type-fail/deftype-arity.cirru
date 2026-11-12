
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Pair $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Pair (:generics A B) (Tuple A B)
          :examples $ []
        'Vec2 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Vec2 (Tuple Number Number)
          :examples $ []
        'first-of $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn first-of (p) (nth p 0)
          :examples $ []
          :schema $ quote $ Fn (Pair Number) (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            first-of $ [] 1 |a
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
