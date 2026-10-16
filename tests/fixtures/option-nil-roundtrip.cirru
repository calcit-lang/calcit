
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|只读 Option→nil 源码证据与共享语义回归") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|WASI 0.3 command Component 退出码集成测试入口。")
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|开发模式重载占位入口。")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
    'app.other $ %{} 'FileEntry
      :defs $ {} $ 'unwrap-value
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unwrap-value (value) (.unwrap-or value nil)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Option 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.other
    'app.roundtrip $ %{} 'FileEntry
      :defs $ {}
        'Other $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Other
            defenum Other (:some 'Dynamic) (:none)
          :examples $ []
          :schema $ :: 'EnumDef
        'match-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn match-value (value)
            match value
              (:some item) item
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |keeps-absence)
              :code $ quote $ assert= nil
                match-value $ Option :none
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-present-nil)
              :code $ quote $ assert= nil
                match-value $ Option :some nil
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-false)
              :code $ quote $ assert= false
                match-value $ Option :some false
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-zero)
              :code $ quote $ assert= 0
                match-value $ Option :some 0
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-empty)
              :code $ quote $ assert= |
                match-value $ Option :some |
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-unicode)
              :code $ quote $ assert= "|中文"
                match-value $ Option :some "|中文"
              :tags $ #{} :option-nil-roundtrip :unit
        'other-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn other-match (value)
            match value
              (:some item) item
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Other
        'quoted-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn quoted-value ()
            quote $ .unwrap-or
              get ({}) :name
              , nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'read-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-field (data)
            either
              .unwrap-or (get data :name) nil
              , |fallback
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |missing-fallback)
              :code $ quote $ assert= |fallback
                read-field $ {}
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |nil-fallback)
              :code $ quote $ assert= |fallback
                read-field $ {} $ :name nil
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |false-is-not-absence)
              :code $ quote $ assert= false
                read-field $ {} $ :name false
              :tags $ #{} :option-nil-roundtrip :unit
        'reader-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reader-value (state)
            match @state
              (:some item) item
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Ref (:: 'Option 'Dynamic)
          :tests $ []
            %{} 'TestEntry (:name |reader-absence)
              :code $ quote $ assert= nil
                reader-value $ atom $ Option :none
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |reader-present-nil)
              :code $ quote $ assert= nil
                reader-value $ atom $ Option :some nil
              :tags $ #{} :option-nil-roundtrip :unit
        'threaded-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn threaded-value (value) (value .unwrap-or nil)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |keeps-absence)
              :code $ quote $ assert= nil
                threaded-value $ Option :none
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-present-nil)
              :code $ quote $ assert= nil
                threaded-value $ Option :some nil
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-false)
              :code $ quote $ assert= false
                threaded-value $ Option :some false
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-zero)
              :code $ quote $ assert= 0
                threaded-value $ Option :some 0
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-empty)
              :code $ quote $ assert= |
                threaded-value $ Option :some |
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-unicode)
              :code $ quote $ assert= "|中文"
                threaded-value $ Option :some "|中文"
              :tags $ #{} :option-nil-roundtrip :unit
        'unwrap-value $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn unwrap-value (value) (.unwrap-or value nil)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Option 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |keeps-absence)
              :code $ quote $ assert= nil
                unwrap-value $ Option :none
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-present-nil)
              :code $ quote $ assert= nil
                unwrap-value $ Option :some nil
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-false)
              :code $ quote $ assert= false
                unwrap-value $ Option :some false
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-zero)
              :code $ quote $ assert= 0
                unwrap-value $ Option :some 0
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-empty)
              :code $ quote $ assert= |
                unwrap-value $ Option :some |
              :tags $ #{} :option-nil-roundtrip :unit
            %{} 'TestEntry (:name |keeps-unicode)
              :code $ quote $ assert= "|中文"
                unwrap-value $ Option :some "|中文"
              :tags $ #{} :option-nil-roundtrip :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.roundtrip
