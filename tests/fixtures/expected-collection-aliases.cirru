
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |demo
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'demo.main/main!) (:mode :native) (:reload-fn 'demo.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'demo.main
    %{} 'FileEntry
      :defs $ {}
        'Color $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Color (:red) (:green) (:rgb Number Number Number)
          :examples $ []
          :schema $ quote EnumDef
        'ColorSlots $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype ColorSlots (:generics T) (Map Color T)
          :examples $ []
        'Palette $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype Palette (List Color)
          :examples $ []
        'lookup-aliased-color $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lookup-aliased-color (xs)
            .unwrap $ &map:get xs $ Color :red
          :examples $ []
          :schema $ quote $ Fn (ColorSlots Number) (-> Number)
          :tests $ [] $ %{} 'TestEntry (:name |infers-enum-key-through-generic-map-alias)
            :code $ quote $ assert= 7
              lookup-aliased-color $ &{} :red 7
            :tags $ #{} :unit
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            assert=
              [] (Color :red) (Color :green)
              tag-aliased-list-return
            assert= 7 $ lookup-aliased-color $ &{} :red 7
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'tag-aliased-list-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-aliased-list-return () ([] :red :green)
          :examples $ []
          :schema $ quote $ Fn (-> Palette)
          :tests $ [] $ %{} 'TestEntry (:name |infers-enum-members-through-list-alias)
            :code $ quote $ assert=
              [] (Color :red) (Color :green)
              tag-aliased-list-return
            :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
