
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-impl-missing-required-trait-strict
  :entries $ {} $ :default
    {}
      :description "|Strict fixture for attaching a child trait impl without its parent impl."
      :init-fn 'type-fail-impl-missing-required-trait-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-impl-missing-required-trait-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-impl-missing-required-trait-strict.main
    %{} 'FileEntry
      :defs $ {}
        'Child $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Child ('requires Parent) (.greet :fn)
          :examples $ []
          :schema $ :: 'Trait
        'ChildImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl ChildImpl Child
            .greet $ fn (x) |ok
          :examples $ []
          :schema $ :: 'Impl
        'Parent $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Parent (.label :fn)
          :examples $ []
          :schema $ :: 'Trait
        'Widget $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def Widget
            impl-traits
              defstruct Widget $ :name 'String
              , ChildImpl
          :examples $ []
          :schema $ :: 'StructDef
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                refs $ [] Widget
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
        :doc "|Strict fixture for attaching a child trait impl without its parent impl."
        :code $ quote $ ns type-fail-impl-missing-required-trait-strict.main
