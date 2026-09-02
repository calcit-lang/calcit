
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |test-method-errors)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-method-errors.main/main!) (:mode :native) (:reload-fn 'test-method-errors.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'test-method-errors.main $ %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|Entry for reproducing preprocess failures")
          :code $ quote
            defn main! () (; "运行该入口会在" preprocess "阶段报错，验证类型推断是否生效")
              trigger-type-error $ {} (:a 1) (:b 2)
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        'trigger-type-error $ %{} 'CodeEntry (:doc "|Pipeline sample that should fail preprocess type checks")
          :code $ quote
            defn trigger-type-error (src)
              do (.to-set src)
                src .map $ fn (x) false
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
      :ns $ %{} 'NsEntry (:doc "|Namespace for standalone repro")
        :code $ quote (ns test-method-errors.main)
