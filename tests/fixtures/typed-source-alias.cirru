
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |fix-command
  :entries $ {} $ :default
    {} (:description "|公开名称的依赖模块 fixture；仅通过 consumer 使用") (:init-fn 'fix-command.reader/main!) (:mode :native) (:reload-fn 'fix-command.reader/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'fix-command.alias-evidence $ %{} 'FileEntry
      :defs $ {}
        'apply-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def apply-alias fix-command.alias-evidence/apply-text
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |callback-context)
            :code $ quote $ do
              assert= |ok $ fix-command.alias-evidence/apply-alias fix-command.alias-evidence/echo-chain |ok
              assert= |hello! $ fix-command.alias-evidence/apply-alias
                fn (text) (&str:concat text |!)
                , |hello
            :tags $ #{} :alias-contract :unit
        'apply-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-text (f x)
            hint-fn $ {} (:return 'String)
              :args $ []
                :: 'Fn $ {} (:return 'String)
                  :args $ [] 'String
                , 'String
            f x
          :examples $ []
        'echo-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def echo-alias fix-command.alias-evidence/echo-string
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
          :tests $ [] $ %{} 'TestEntry (:name |direct-and-local-values)
            :code $ quote $ let
                local-f fix-command.alias-evidence/echo-alias
              assert= |ok $ fix-command.alias-evidence/echo-alias |ok
              assert= |ok $ fix-command.alias-evidence/echo-chain |ok
              assert= |ok $ local-f |ok
              assert= ([] |a |b)
                map ([] |a |b) fix-command.alias-evidence/echo-chain
            :tags $ #{} :alias-contract :unit
        'echo-chain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def echo-chain fix-command.alias-evidence/echo-alias
          :examples $ []
        'echo-string $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn echo-string (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'optional-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def optional-alias fix-command.alias-evidence/optional-text
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |optional-values)
            :code $ quote $ do
              assert= |fallback $ fix-command.alias-evidence/optional-alias
              assert= |ok $ fix-command.alias-evidence/optional-alias $ Option :some |ok
            :tags $ #{} :alias-contract :unit
        'optional-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn optional-text (x)
            hint-fn $ {} (:return 'String)
              :args $ [] $ :: 'Option 'String
            .unwrap-or x |fallback
          :examples $ []
        'rest-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def rest-alias fix-command.alias-evidence/rest-count
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |rest-values)
            :code $ quote $ do
              assert= 0 $ fix-command.alias-evidence/rest-alias |first
              assert= 2 $ fix-command.alias-evidence/rest-alias |first |second |third
            :tags $ #{} :alias-contract :unit
        'rest-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-count (x & more)
            hint-fn $ {} (:return 'Number) (:rest 'String)
              :args $ [] 'String
            &list:count more
          :examples $ []
        'same-alias $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def same-alias fix-command.alias-evidence/same-first
          :examples $ []
          :tests $ [] $ %{} 'TestEntry (:name |generic-relations)
            :code $ quote $ do
              assert= 3 $ fix-command.alias-evidence/same-alias 3 4
              assert= |a $ fix-command.alias-evidence/same-alias |a |b
            :tags $ #{} :alias-contract :unit
        'same-first $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn same-first (left right)
            hint-fn $ {} (:return 'T)
              :generics $ [] 'T
              :args $ [] 'T 'T
            , left
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.alias-evidence
    'fix-command.reader $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'read-dir $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-dir (path) (&str:concat |reader:dir: path)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
        'read-file $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn read-file (path) (&str:concat |reader:file: path)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ [] 'String
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns fix-command.reader
