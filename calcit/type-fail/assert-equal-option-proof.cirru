
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-assert-equal-option-proof
  :entries $ {} $ :default
    {}
      :description "|Assertion-proof fixture: assert= erases both sides to Dynamic before comparing Option values."
      :init-fn 'type-fail-assert-equal-option-proof.main/main!
      :mode :native
      :reload-fn 'type-fail-assert-equal-option-proof.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-assert-equal-option-proof.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|assert= 比较字面 Option 与 get 返回的 Option；两侧都被显式擦除为 Dynamic，不是可空值的旧用法。"
          :code $ quote $ defn main! ()
            assert= (Option :none) $ get ([] 1) 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-assert-equal-option-proof.main
