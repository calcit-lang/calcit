
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-slice-receiver-trait
  :entries $ {} $ :default
    {} (:description |)
      :init-fn 'type-fail-slice-receiver-trait.main/main!
      :mode :native
      :reload-fn 'type-fail-slice-receiver-trait.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-slice-receiver-trait.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry
          :doc "|Reject a receiver that does not implement Sliceable"
          :code $ quote $ defn main! () (slice 1 0 1) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote $ defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Namespace for slice receiver trait mismatch"
        :code $ quote $ ns type-fail-slice-receiver-trait.main
