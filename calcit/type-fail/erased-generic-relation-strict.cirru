
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |type-fail-erased-generic-relation-strict)
  :entries $ {}
    :default $ {} (:description "|Strict preprocessing fixture for a generic relation erased by Dynamic.") (:init-fn 'type-fail-erased-generic-relation-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-erased-generic-relation-strict.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'type-fail-erased-generic-relation-strict.main $ %{} 'FileEntry
      :defs $ {}
        'compare-open $ %{} 'CodeEntry (:doc "|验证开放 Dynamic 绑定不能被只接受 Number 的 callback 隐式收窄。")
          :code $ quote
            defn compare-open (value callback) (callback value)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'T)
              :args $ [] 'T
                :: 'Fn $ {} (:return 'T)
                  :args $ [] 'T
              :generics $ [] 'T
        'main! $ %{} 'CodeEntry (:doc "|入口构造 Dynamic 输入与 Number-only callback，要求严格预处理报告开放泛型绑定收窄。")
          :code $ quote
            defn main! () $ compare-open (assert-type 1 'Dynamic)
              fn (value)
                hint-fn $ {}
                  :args $ [] 'Number
                  :return 'Number
                + value 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict erased-generic-relation fixture.")
        :code $ quote (ns type-fail-erased-generic-relation-strict.main)
