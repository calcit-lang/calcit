
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-trait-requires-kind-mismatch-strict
  :entries $ {} $ :default
    {}
      :description "|Strict fixture for an external-object trait requiring an ordinary trait."
      :init-fn 'type-fail-trait-requires-kind-mismatch-strict.main/main!
      :mode :js
      :reload-fn 'type-fail-trait-requires-kind-mismatch-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-trait-requires-kind-mismatch-strict.main
    %{} 'FileEntry
      :defs $ {}
        'HostChild $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait HostChild ('requires Plain) (:size 'Number)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)
          :schema $ :: 'Trait
        'Plain $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Plain (.label :fn)
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                refs $ [] HostChild
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry
        :doc "|Strict fixture for an external-object trait requiring an ordinary trait."
        :code $ quote $ ns type-fail-trait-requires-kind-mismatch-strict.main
