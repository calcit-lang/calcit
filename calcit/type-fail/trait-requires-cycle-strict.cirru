
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-trait-requires-cycle-strict
  :entries $ {} $ :default
    {}
      :description "|Strict fixture for a cycle between required traits."
      :init-fn 'type-fail-trait-requires-cycle-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-trait-requires-cycle-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-trait-requires-cycle-strict.main
    %{} 'FileEntry
      :defs $ {}
        'TraitA $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait TraitA (requires TraitB) (.a :fn)
          :examples $ []
          :schema $ :: 'Trait
        'TraitB $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait TraitB (requires TraitA) (.b :fn)
          :examples $ []
          :schema $ :: 'Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                refs $ [] TraitA TraitB
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
        :doc "|Strict fixture for a cycle between required traits."
        :code $ quote $ ns type-fail-trait-requires-cycle-strict.main
