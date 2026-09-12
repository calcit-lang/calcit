
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |fix-command)
  :entries $ {}
    :default $ {} (:description "|Compiler-guided source fix fixture.") (:init-fn 'fix-command.main/main!) (:mode :native) (:reload-fn 'fix-command.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'fix-command.main $ %{} 'FileEntry
      :defs $ {}
        'ambiguous $ %{} 'CodeEntry (:doc "|Removed predicate needs a semantic choice.")
          :code $ quote
            defn ambiguous (value) (tuple? value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        'fixable $ %{} 'CodeEntry (:doc "|Exact removed API call.")
          :code $ quote
            defn fixable (value) (tuple-enum value)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ [] 'Dynamic
              :return $ :: 'Option 'EnumDef
          :tests $ []
            %{} 'TestEntry (:name |returns-enum-definition)
              :code $ quote
                assert= (%some Option)
                  fixable $ %some 1
              :tags $ #{} :migration
        'main! $ %{} 'CodeEntry (:doc |Entry.)
          :code $ quote
            defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'shadowed $ %{} 'CodeEntry (:doc "|A local binding that shares the removed core API name.")
          :code $ quote
            defn shadowed (tuple-enum value) (tuple-enum value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
                :: 'Fn $ {} (:return 'Dynamic)
                  :args $ [] 'Dynamic
                , 'Dynamic
        'tag-match-case $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn tag-match-case (is-some?)
              let
                  value $ if is-some? (:: :some 1) (:: :none)
                tag-match value
                  (:some x) (+ x 1)
                  (:none) 0
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Bool
          :tests $ []
            %{} 'TestEntry (:name |preserves-some-payload)
              :code $ quote
                assert= 2 $ tag-match-case true
              :tags $ #{} :migration
            %{} 'TestEntry (:name |preserves-none-branch)
              :code $ quote
                assert= 0 $ tag-match-case false
              :tags $ #{} :migration
        'tag-match-quoted $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn tag-match-quoted () $ quote
              tag-match (%some 1)
                (:some x) x
                (:none) 0
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'CirruQuote)
              :args $ []
          :tests $ []
            %{} 'TestEntry (:name |keeps-quoted-data)
              :code $ quote
                assert=
                  quote $ tag-match (%some 1)
                    (:some x) x
                    (:none) 0
                  tag-match-quoted
              :tags $ #{} :migration
        'tag-match-shadowed $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn tag-match-shadowed (tag-match value) (tag-match value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
                :: 'Fn $ {} (:return 'Number)
                  :args $ [] 'Number
                , 'Number
          :tests $ []
            %{} 'TestEntry (:name |keeps-local-shadow)
              :code $ quote
                assert= 11 $ tag-match-shadowed
                  fn (value) (+ value 10)
                  , 1
              :tags $ #{} :migration
      :ns $ %{} 'NsEntry (:doc "|Fix command fixture.")
        :code $ quote (ns fix-command.main)
