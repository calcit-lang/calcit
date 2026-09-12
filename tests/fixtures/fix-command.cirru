{} (:about "|Fix command integration fixture.") (:package |fix-command)
  :entries $ {}
    :default $ {} (:description "|Compiler-guided source fix fixture.") (:init-fn 'fix-command.main/main!) (:mode :native) (:reload-fn 'fix-command.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'fix-command.main $ %{} 'FileEntry
      :defs $ {}
        'fixable $ %{} 'CodeEntry (:doc "|Exact removed API call.")
          :code $ quote
            defn fixable (value) (tuple-enum value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return $ :: 'Option 'EnumDef)
              :args $ [] 'Dynamic
          :tests $ []
            %{} 'TestEntry (:name |returns-enum-definition)
              :code $ quote
                assert= (%some Option)
                  fixable $ %some 1
              :tags $ #{} :migration
        'ambiguous $ %{} 'CodeEntry (:doc "|Removed predicate needs a semantic choice.")
          :code $ quote
            defn ambiguous (value) (tuple? value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc "|Entry.")
          :code $ quote
            defn main! () &unit
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
              :args $ [] (:: 'Fn $ {} (:args $ [] 'Dynamic) (:return 'Dynamic)) 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Fix command fixture.")
        :code $ quote (ns fix-command.main)
