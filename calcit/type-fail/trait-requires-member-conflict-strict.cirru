
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-trait-requires-member-conflict-strict
  :entries $ {} $ :default
    {}
      :description "|Strict fixture for a child trait redeclaring a parent member."
      :init-fn 'type-fail-trait-requires-member-conflict-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-trait-requires-member-conflict-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-trait-requires-member-conflict-strict.main
    %{} 'FileEntry
      :defs $ {}
        'Child $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Child (requires Parent) (.label :fn)
          :examples $ []
          :schema $ :: 'Trait
        'Parent $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Parent (.label :fn)
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                refs $ [] Child
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
        :doc "|Strict fixture for a child trait redeclaring a parent member."
        :code $ quote $ ns type-fail-trait-requires-member-conflict-strict.main
